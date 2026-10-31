// What Choco does with a sentence. Plain commands are matched here, offline and instantly;
// anything else goes to the AI service in use (ai.rs), which answers and may call the same actions.
import { invoke } from '@tauri-apps/api/core';
import { focus, LABEL } from './focus.svelte';
import { nowPlaying } from './media.svelte';
import { status } from './status.svelte';
import { hhmm, mmss } from './store';
import { todos } from './todos.svelte';
import { water } from './water.svelte';

/** A live card under the reply. */
export type Panel = 'tasks' | 'water' | 'timer' | 'music';
/** `computer` is a task for Choco to do on the desktop (see computer.rs). */
export type Reply = { text: string; tone?: 'bad'; panel?: Panel; computer?: string };
export type Turn = { role: 'user' | 'assistant'; content: string };
type Action = { name: string; input: Record<string, unknown> };

const PANELS: Panel[] = ['tasks', 'water', 'timer', 'music'];

/** Run one island action and say what happened. */
function act({ name, input }: Action): Reply {
  switch (name) {
    case 'focus_timer': {
      const a = input.action;
      if (a === 'start_focus' || a === 'start_break') {
        const kind = a === 'start_focus' ? 'focus' : 'break';
        if (focus.kind !== kind) focus.choose(kind);
        focus.start();
        return { text: `${LABEL[kind]}, ${mmss(focus.remaining(Date.now()))} on the clock.`, panel: 'timer' };
      }
      if (a === 'pause') {
        focus.pause();
        return { text: 'Paused. Pick it up whenever you like.', panel: 'timer' };
      }
      focus.reset();
      return { text: 'Timer reset.', panel: 'timer' };
    }
    case 'add_task': {
      const due = typeof input.due === 'string' && /^\d{2}:\d{2}$/.test(input.due) ? input.due : null;
      todos.add(String(input.text ?? ''), due);
      return { text: due ? `Added, I'll remind you at ${due}.` : 'Added to your list.', panel: 'tasks' };
    }
    case 'drink_water':
      water.drink();
      return { text: water.percent >= 100 ? 'Goal reached for today. Nice.' : 'Logged a cup.', panel: 'water' };
    case 'media': {
      const a = input.action;
      const playing = !!nowPlaying.media?.playing;
      if ((a === 'play' && !playing) || (a === 'pause' && playing)) nowPlaying.togglePlay();
      else if (a === 'next') nowPlaying.next();
      else if (a === 'previous') nowPlaying.previous();
      const text = a === 'pause' ? 'Paused.' : a === 'play' ? 'Playing.' : a === 'next' ? 'Skipped ahead.' : 'Going back.';
      return { text, panel: nowPlaying.media ? 'music' : undefined };
    }
    case 'set_volume': {
      const level = Math.max(0, Math.min(100, Math.round(Number(input.level) || 0)));
      status.setVolume(level);
      return { text: `Volume at ${level}.` };
    }
    case 'set_mute': {
      if (!!status.current?.muted !== !!input.muted) status.toggleMute();
      return { text: input.muted ? 'Muted.' : 'Sound back on.' };
    }
    case 'show': {
      const panel = PANELS.find((p) => p === input.panel);
      return { text: '', panel };
    }
  }
  return { text: '' };
}

/** "5", "30", "pm" → "17:30". Without am/pm, picks the next time it will be that o'clock. */
function clockTime(h: number, m: number, meridiem?: string) {
  if (meridiem === 'pm' && h < 12) h += 12;
  if (meridiem === 'am' && h === 12) h = 0;
  if (!meridiem && h < 12) {
    const now = new Date();
    if (h * 60 + m <= now.getHours() * 60 + now.getMinutes()) h += 12;
  }
  return `${String(h % 24).padStart(2, '0')}:${String(m).padStart(2, '0')}`;
}

const VOLUME_STEP = 10;

/** Sentences we understand without asking anyone. Patterns cover the whole sentence,
    so "start a timer and add a task" isn't half-done; the model gets those. */
function offline(s: string): Action | Reply | null {
  let m: RegExpMatchArray | null;
  if (/^(start|begin)( a| the)? (focus|pomodoro|timer|focus session|focus timer|session)$/.test(s) || s === 'focus')
    return { name: 'focus_timer', input: { action: 'start_focus' } };
  if (/^((start|take)( a)? )?(short )?break$/.test(s)) return { name: 'focus_timer', input: { action: 'start_break' } };
  if (/^(pause|stop)( the)? (timer|focus|session)$/.test(s)) return { name: 'focus_timer', input: { action: 'pause' } };
  if (/^reset( the)? (timer|focus|session)$/.test(s)) return { name: 'focus_timer', input: { action: 'reset' } };

  if ((m = s.match(/^(?:add(?: a)? (?:task|todo|to do)(?: to)?|remind me to|new task) (.+?)(?: at (\d{1,2})(?:[: ](\d{2}))? ?(am|pm)?)?$/))) {
    const due = m[2] ? clockTime(+m[2], m[3] ? +m[3] : 0, m[4]) : '';
    return { name: 'add_task', input: { text: m[1], due } };
  }

  if (/^(i )?(drank|had|log|add)( a| one| another)? (cup|glass)( of)? water$|^(log|drink|drank) water$/.test(s))
    return { name: 'drink_water', input: {} };

  if (/^(play|resume)( the)?( music| song)?$/.test(s)) return { name: 'media', input: { action: 'play' } };
  if (/^(pause|stop)( the)?( music| song)?$/.test(s)) return { name: 'media', input: { action: 'pause' } };
  if (/^(next|skip)( the)?( song| track)?$/.test(s)) return { name: 'media', input: { action: 'next' } };
  if (/^(previous|last|go back)( song| track)?$/.test(s)) return { name: 'media', input: { action: 'previous' } };

  if ((m = s.match(/^(?:set )?(?:the )?volume(?: to)? (\d{1,3})(?: percent)?$/)))
    return { name: 'set_volume', input: { level: +m[1] } };
  if (/^(turn )?(the )?volume up$|^louder$/.test(s))
    return { name: 'set_volume', input: { level: (status.current?.volume ?? 50) + VOLUME_STEP } };
  if (/^(turn )?(the )?volume down$|^quieter$/.test(s))
    return { name: 'set_volume', input: { level: (status.current?.volume ?? 50) - VOLUME_STEP } };
  if (s === 'mute') return { name: 'set_mute', input: { muted: true } };
  if (s === 'unmute') return { name: 'set_mute', input: { muted: false } };

  if (/^what( is|s) the time$|^what time is it$/.test(s)) return { text: `It's ${hhmm(Date.now())}.` };
  if (/^(show|what s on|whats on|what is on)( me)?( my)? (tasks|to do|todo|list|to do list)$/.test(s)) return { text: '', panel: 'tasks' };
  if (/^(show|how much)( my)? water( today)?$/.test(s)) return { text: '', panel: 'water' };
  return null;
}

const words = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim();

/** "open spotify": an app from the Start menu by name, opened straight away. Null when no
    app goes by that name, so the model can take it (a website, a file, a setting). */
async function openApp(s: string): Promise<Reply | null> {
  const m = s.match(/^(?:open|launch|start|run)(?: up)? (?:the |my )?(.+?)(?: app)?$/);
  if (!m) return null;
  const want = m[1];
  let apps: { name: string; id: string }[];
  try {
    apps = await invoke('apps_list');
  } catch {
    return null;
  }
  const named = apps.map((a) => ({ ...a, words: words(a.name) }));
  const app =
    named.find((a) => a.words === want) ??
    named.find((a) => a.words.startsWith(want + ' ')) ??
    named.find((a) => a.words.split(' ').includes(want) && want.length > 3);
  if (!app) return null;
  try {
    await invoke('apps_launch', { id: app.id });
    return { text: `Opening ${app.name}.` };
  } catch (e) {
    return { text: String(e), tone: 'bad' };
  }
}

/** One line about what's going on, so the model can answer "how long is left?" and the like. */
function context() {
  const now = new Date();
  const m = nowPlaying.media;
  const open = todos.tasks.filter((t) => !t.doneOn).map((t) => (t.due ? `${t.text} (${t.due})` : t.text));
  return [
    `It is ${now.toLocaleString('en-GB', { weekday: 'long', day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' })}.`,
    focus.running
      ? `${LABEL[focus.kind]} timer running, ${mmss(focus.remaining(Date.now()))} left.`
      : 'The focus timer is not running.',
    m ? `Music: "${m.title}" by ${m.artist || 'unknown'}, ${m.playing ? 'playing' : 'paused'}.` : 'No music playing.',
    open.length ? `Open tasks today: ${open.join('; ')}.` : 'No open tasks today.',
    `Water today: ${water.ml} of ${water.goalMl} ml.`,
    status.current ? `Volume ${status.current.volume}${status.current.muted ? ' (muted)' : ''}.` : '',
  ]
    .filter(Boolean)
    .join(' ');
}

/** Speech-to-text output → what to do and what to show. Null when nothing was said. */
export async function handle(heard: string, history: Turn[], session: string): Promise<Reply | null> {
  const s = heard
    .toLowerCase()
    .replace(/(\d)\.(\d)/g, '$1:$2') // "5.30" → "5:30"
    .replace(/a\.m\./g, 'am')
    .replace(/p\.m\./g, 'pm')
    .replace(/[^a-z0-9: ]+/g, ' ')
    .replace(/^(hey |ok |okay )?choco\b/, '')
    .replace(/\b(please|can you|could you)\b/g, '')
    .replace(/\s+/g, ' ')
    .trim();
  if (!s) return null;

  const local = offline(s);
  if (local) return 'name' in local ? act(local) : local;
  const opened = await openApp(s);
  if (opened) return opened;

  try {
    const answer = await invoke<{ reply: string; actions: Action[] }>('choco_ask', {
      text: heard,
      context: context(),
      history,
      session,
    });
    const task = answer.actions.find((a) => a.name === 'use_computer')?.input.task;
    const done = answer.actions.map(act);
    if (typeof task === 'string' && task) return { text: '', computer: task };
    // The model's own words win; an action's summary stands in if it said nothing.
    const text = answer.reply || done.findLast((d) => d.text)?.text || 'Done.';
    return { text, panel: done.findLast((d) => d.panel)?.panel };
  } catch (e) {
    const msg = String(e);
    if (msg === 'no-key') return { text: 'I can only do the basics until there’s an AI key in Settings.', tone: 'bad' };
    if (msg.startsWith('network')) return { text: 'I can’t reach the internet right now.', tone: 'bad' };
    return { text: msg, tone: 'bad' };
  }
}
