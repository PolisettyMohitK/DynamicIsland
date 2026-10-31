// "Hey Choco": the backend listens for the wake word and transcribes what follows;
// this turns that into actions and keeps what the island shows.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { popup } from './alerts';
import { handle, type Reply, type Turn } from './commands';
import { load, save } from './store';

type BackendEvent =
  | { state: 'loading' | 'ready' | 'off' | 'thinking' | 'idle' }
  | { state: 'listening'; followUp: boolean }
  | { state: 'heard'; text: string }
  | { state: 'error'; message: string };

type ComputerEvent = { kind: 'step'; text: string } | { kind: 'confirm'; question: string };

export type ChocoState = 'off' | 'loading' | 'ready' | 'listening' | 'thinking' | 'working' | 'answer';
export type Sensitivity = 'low' | 'normal' | 'high';

export type ChocoSettings = {
  enabled: boolean;
  sensitivity: Sensitivity;
  /** Microphone name, or null for the Windows default. */
  mic: string | null;
  /** Listen for a follow-up after answering, without the wake word. */
  followUps: boolean;
};

const THRESHOLD: Record<Sensitivity, number> = { low: 0.7, normal: 0.5, high: 0.3 };

// How long an answer stays once it has appeared: a base plus reading time per word.
const READ_BASE_MS = 3000;
const READ_PER_WORD_MS = 260;
const READ_MAX_MS = 30_000;
/** After the cursor leaves an answer it was holding open. */
const AFTER_HOVER_MS = 1500;
/** A conversation (for follow-ups) ends after this long without a question. */
const CONVERSATION_MS = 2 * 60_000;
const HISTORY_TURNS = 8;

class Choco {
  state = $state<ChocoState>('off');
  /** Mic loudness 0–1 while listening. */
  level = $state(0);
  heard = $state('');
  reply = $state<Reply | null>(null);
  /** The answer is up and Choco is listening for a follow-up. */
  followUp = $state(false);
  /** While working the desktop: what it's doing now, and a Yes / No it's waiting on. */
  step = $state('');
  question = $state<string | null>(null);
  settings = $state<ChocoSettings>(
    load('choco', { enabled: true, sensitivity: 'normal', mic: null, followUps: true } satisfies ChocoSettings),
  );

  #timer: ReturnType<typeof setTimeout> | undefined;
  #hovered = false;
  #readMs = 0;
  #history: Turn[] = [];
  #lastAsked = 0;
  #session = '';
  /** Counts desktop tasks, so a stopped one's late result is dropped. */
  #run = 0;
  /** A "Hey Choco" just stopped a desktop task; "stop" after it means only that. */
  #stopped = false;

  /** Listening or answering: the island shows Choco instead of anything else. */
  get busy() {
    return this.state === 'listening' || this.state === 'thinking' || this.state === 'working' || this.state === 'answer';
  }

  start() {
    const stop = [
      listen<BackendEvent>('choco', (e) => this.#on(e.payload)),
      listen<number>('choco-level', (e) => (this.level = e.payload)),
      listen('choco-toggle', () => this.set({ enabled: !this.settings.enabled }, true)),
      listen<ComputerEvent>('computer', (e) => {
        if (e.payload.kind === 'step') {
          this.step = e.payload.text;
          this.question = null;
        } else this.question = e.payload.question;
      }),
    ];
    this.#apply();
    return () => stop.forEach((p) => p.then((off) => off()));
  }

  /** Change settings; `announce` shows a pop-up for on/off. */
  set(patch: Partial<ChocoSettings>, announce = false) {
    Object.assign(this.settings, patch);
    save('choco', this.settings);
    this.#apply();
    if (announce && 'enabled' in patch) {
      popup({ icon: 'mic', label: 'Hey Choco', value: this.settings.enabled ? 'On' : 'Off', tone: this.settings.enabled ? 'ok' : undefined });
    }
  }

  #apply() {
    const s = this.settings;
    invoke('choco_configure', { threshold: THRESHOLD[s.sensitivity], mic: s.mic });
    invoke('choco_set_enabled', { on: s.enabled });
  }

  /** The cursor is over the answer: keep it up until it leaves. */
  hold(on: boolean) {
    this.#hovered = on;
    if (this.state !== 'answer' || this.#readMs === 0) return;
    clearTimeout(this.#timer);
    if (!on) this.#timer = setTimeout(() => this.dismiss(), AFTER_HOVER_MS);
  }

  /** The answer has finished appearing: start the reading clock, and listen for a follow-up. */
  revealed() {
    if (this.state !== 'answer' || !this.reply) return;
    const words = this.reply.text.split(/\s+/).length + (this.reply.panel ? 6 : 0);
    this.#readMs = Math.min(READ_MAX_MS, READ_BASE_MS + words * READ_PER_WORD_MS);
    if (!this.#hovered) this.#timer = setTimeout(() => this.dismiss(), this.#readMs);
    if (this.settings.followUps && this.reply.tone !== 'bad') invoke('choco_follow_up');
  }

  /** Yes / No to what the desktop task asked. */
  answerQuestion(yes: boolean) {
    this.question = null;
    invoke('computer_answer', { yes });
  }

  stopWork() {
    invoke('computer_stop');
  }

  dismiss() {
    clearTimeout(this.#timer);
    if (this.state === 'answer') this.state = this.settings.enabled ? 'ready' : 'off';
    this.followUp = false;
  }

  async #on(e: BackendEvent) {
    switch (e.state) {
      case 'listening':
        if (e.followUp && this.state === 'answer') {
          this.followUp = true;
          return;
        }
        clearTimeout(this.#timer);
        // Calling Choco while it works the desktop stops that.
        if (this.state === 'working') {
          this.stopWork();
          this.#stopped = true;
        }
        this.heard = '';
        this.reply = null;
        this.level = 0;
        this.followUp = false;
        this.state = 'listening';
        break;
      case 'thinking':
        clearTimeout(this.#timer);
        this.followUp = false;
        this.state = 'thinking';
        break;
      case 'idle':
        // A follow-up nobody used: the answer just stays until it's been read.
        if (this.followUp) this.followUp = false;
        else if (this.state === 'listening') this.state = 'ready';
        break;
      case 'heard': {
        this.heard = e.text;
        const stopping = this.#stopped;
        this.#stopped = false;
        if (stopping && /^\W*((hey|ok|okay)\W+)?(choco\W+)?(stop|cancel|never ?mind)\b/i.test(e.text)) {
          this.#answer({ text: 'Stopped.' });
          return;
        }
        if (Date.now() - this.#lastAsked > CONVERSATION_MS) {
          this.#history = [];
          this.#session = `choco-${Date.now()}`;
        }
        this.#lastAsked = Date.now();
        const reply = await handle(e.text, this.#history, this.#session);
        // A new "Hey Choco" while the model was answering wins.
        if (this.state !== 'thinking' || this.heard !== e.text) return;
        if (!reply) {
          this.state = 'ready';
          return;
        }
        if (reply.computer) {
          this.#work(reply.computer, e.text);
          return;
        }
        if (reply.text && reply.tone !== 'bad') {
          this.#history = [...this.#history, { role: 'user' as const, content: e.text }, { role: 'assistant' as const, content: reply.text }].slice(-HISTORY_TURNS);
        }
        this.#answer(reply);
        break;
      }
      case 'error':
        this.heard = '';
        this.#answer({ text: e.message, tone: 'bad' });
        break;
      default:
        // Loading / ready / off. Don't cut an answer short when the mic reopens.
        if (e.state === 'off' || !this.busy) this.state = e.state;
    }
  }

  /** Works the desktop for `task` (computer.rs), then answers with how it went. */
  async #work(task: string, asked: string) {
    const run = ++this.#run;
    this.step = 'Looking';
    this.question = null;
    this.state = 'working';
    let reply: Reply;
    try {
      reply = { text: await invoke<string>('computer_run', { task }) };
    } catch (e) {
      const msg = String(e);
      const text = msg === 'no-key'
        ? 'I need an AI key in Settings for that.'
        : msg.startsWith('network')
          ? 'I can’t reach the internet right now.'
          : msg;
      reply = { text, tone: 'bad' };
    }
    this.question = null;
    if (run !== this.#run || this.state !== 'working') return;
    if (reply.tone !== 'bad') {
      this.#history = [...this.#history, { role: 'user' as const, content: asked }, { role: 'assistant' as const, content: reply.text }].slice(-HISTORY_TURNS);
    }
    this.#answer(reply);
  }

  #answer(reply: Reply) {
    clearTimeout(this.#timer);
    this.#readMs = 0;
    this.reply = reply;
    this.followUp = false;
    this.state = 'answer';
  }
}

export const choco = new Choco();
