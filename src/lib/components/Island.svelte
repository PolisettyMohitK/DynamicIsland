<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { fade, fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { untrack } from 'svelte';
  import { onPopup, type Popup } from '$lib/alerts';
  import { clock } from '$lib/clock.svelte';
  import { focus, LABEL } from '$lib/focus.svelte';
  import { choco } from '$lib/choco.svelte';
  import { dictation } from '$lib/dictation.svelte';
  import { nowPlaying } from '$lib/media.svelte';
  import { pet } from '$lib/pet.svelte';
  import { shelf } from '$lib/shelf.svelte';
  import { status } from '$lib/status.svelte';
  import { mmss } from '$lib/store';
  import { todos } from '$lib/todos.svelte';
  import { serveSettings } from '$lib/settings-host.svelte';
  import { setLyrics, ui } from '$lib/ui.svelte';
  import { water } from '$lib/water.svelte';
  import { ISLAND_TOP, type Shape } from '$lib/glass';
  import Answer from './Answer.svelte';
  import Bars from './Bars.svelte';
  import BatteryRing from './BatteryRing.svelte';
  import ChocoFace, { type Mood } from './ChocoFace.svelte';
  import ControlCentre from './ControlCentre.svelte';
  import Digits from './Digits.svelte';
  import Glass from './Glass.svelte';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';
  import Pill from './Pill.svelte';
  import Settings from './Settings.svelte';
  import Wave from './Wave.svelte';
  import LyricsView from './music/LyricsView.svelte';
  import NowPlaying from './music/NowPlaying.svelte';
  import FocusPage from './pages/FocusPage.svelte';
  import ShelfPage from './pages/ShelfPage.svelte';
  import TodoPage from './pages/TodoPage.svelte';
  import WaterPage from './pages/WaterPage.svelte';

  /** idle and live are resting states (cursor away); the rest are shown on hover or for events. */
  type Mode = 'idle' | 'live' | 'peek' | 'expanded' | 'popup' | 'voice' | 'dictate' | 'work' | 'answer' | 'settings' | 'control';

  // Size of each state in logical px; r is the radius of the bottom corners.
  // `wide` is the expanded island with the lyrics column open. Answers and settings
  // are as tall as what they show.
  const SHAPES: Record<Exclude<Mode, 'answer' | 'settings' | 'control'> | 'wide', Shape> = {
    idle: { w: 100, h: 30, r: 15 },
    live: { w: 390, h: 36, r: 18 },
    peek: { w: 190, h: 28, r: 14 },
    expanded: { w: 545, h: 180, r: 32 },
    wide: { w: 675, h: 180, r: 32 },
    popup: { w: 425, h: 36, r: 18 },
    voice: { w: 320, h: 44, r: 22 },
    dictate: { w: 320, h: 44, r: 22 },
    work: { w: 440, h: 44, r: 22 },
  };
  /** The resting pill while the pet is out on the desktop: its empty bed. */
  const AWAY: Shape = { w: 64, h: 22, r: 11 };
  /** The resting pill while Choco sleeps in it: the time on its left, the day on its right. */
  const SLEEPING: Shape = { w: 184, h: 32, r: 16 };
  const SETTINGS_W = 720;
  const CONTROL_W = 560;
  // Short answers sit in a narrower island than long ones or ones with a card.
  const ANSWER_W = 460;
  const ANSWER_WIDE_W = 540;
  const LONG_ANSWER = 140;
  /** Measured heights round up to this, so the glass isn't rebuilt for every pixel. */
  const HEIGHT_STEP = 8;
  const TIMER_GLOW = '242 137 124';
  // Choco sleeps through the night.
  const NIGHT = { from: 23, to: 7 };

  const PAGES = [
    { name: 'Now Playing', accent: 'var(--accent-music)' },
    { name: 'Focus', accent: 'var(--accent-focus)' },
    { name: 'Water', accent: 'var(--accent-water)' },
    { name: 'Shelf', accent: 'var(--accent-shelf)' },
    { name: 'To-do', accent: 'var(--accent-todo)' },
  ] as const;
  const SHELF_PAGE = 3;
  // How often due tasks and water reminders are checked.
  const REMINDER_EVERY = 20_000;
  /** How long an open island waits after the cursor leaves. */
  const LEAVE_HOLD = 1000;

  let mode = $state<Mode>('idle');
  let page = $state(0);
  let direction = $state(1);
  let popup = $state<Popup>({ icon: 'caps', label: '' });

  const lyricsOn = $derived(ui.lyricsOn);
  let answerH = $state(0);
  let settingsH = $state(0);
  let controlH = $state(0);

  let hovered = false;
  /** A reminder that arrived while the island was open. */
  let missed: Popup | null = null;
  let timers: ReturnType<typeof setTimeout>[] = [];
  let lastWheel = 0;

  const wide = $derived(page === 0 && lyricsOn && nowPlaying.media !== null);
  const petOut = $derived(pet.settings.enabled && !pet.home);
  const answerW = $derived(
    choco.reply?.panel || (choco.reply?.text.length ?? 0) > LONG_ANSWER ? ANSWER_WIDE_W : ANSWER_W,
  );
  /** Choco in the resting pill: asleep at night, when switched off or when the pet is home
      (the island is its bed), bobbing to music. */
  const idleMood = $derived.by<Mood>(() => {
    void mode; // checked again whenever the island changes
    if (pet.settings.enabled || !choco.settings.enabled || isNight()) return 'sleep';
    return nowPlaying.media?.playing ? 'vibe' : 'idle';
  });
  const fit = (h: number) => Math.ceil(h / HEIGHT_STEP) * HEIGHT_STEP;
  const shape = $derived<Shape>(
    mode === 'expanded' && wide
      ? SHAPES.wide
      : mode === 'answer'
        ? { w: answerW, h: fit(Math.max(56, answerH)), r: 28 }
        : mode === 'control'
          ? { w: CONTROL_W, h: fit(Math.max(120, controlH)), r: 36 }
        : mode === 'settings'
          ? { w: SETTINGS_W, h: fit(Math.max(180, settingsH)), r: 32 }
          : mode === 'idle' && petOut
            ? AWAY
            : mode === 'idle' && idleMood === 'sleep'
              ? SLEEPING
              : SHAPES[mode],
  );
  const voice = $derived(mode === 'voice' || mode === 'work' || mode === 'answer');
  const grow = $derived(
    mode === 'expanded' || mode === 'settings' || mode === 'control' || mode === 'popup' || mode === 'live' || mode === 'dictate' || voice,
  );
  /** Where the island settles when nothing else is going on. */
  const rest = $derived<Mode>(focus.running || (nowPlaying.media?.playing && lyricsOn) ? 'live' : 'idle');
  /** A running focus timer takes the live bar over from music. */
  const liveTimer = $derived(focus.running);
  const glowColour = $derived(liveTimer && mode === 'live' ? TIMER_GLOW : nowPlaying.colour);
  const glow = $derived(glowColour !== null && ((mode === 'expanded' && wide) || mode === 'live'));
  const liveText = $derived.by(() => {
    if (liveTimer) return LABEL[focus.kind];
    const m = nowPlaying.media;
    if (!m) return '';
    if (nowPlaying.lyricsState !== 'ready') return [m.title, m.artist].filter(Boolean).join(' — ');
    return nowPlaying.lyrics[nowPlaying.line]?.text ?? '♪';
  });
  const now = $derived(new Date(clock.now));
  const date = $derived(now.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' }));
  const day = $derived(now.toLocaleDateString('en-GB', { weekday: 'short' }));
  const time = $derived(now.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' }));
  const current = $derived(PAGES[page]);

  /** iOS-style arrival: content comes into focus out of the glass rather than just fading. */
  function materialise(_: Element, { duration = 200, delay = 0 } = {}) {
    return {
      duration,
      delay,
      easing: cubicOut,
      css: (t: number) => `opacity: ${t}; filter: blur(${(1 - t) * 4}px); scale: ${0.96 + 0.04 * t}`,
    };
  }

  function isNight() {
    const hour = new Date().getHours();
    return hour >= NIGHT.from || hour < NIGHT.to;
  }

  function toggleLyrics() {
    setLyrics(!ui.lyricsOn);
  }

  /** Run a timeline of [mode, atMs] steps, cancelling whatever timeline was running.
      'rest' resolves when its step fires, so it lands on the right resting state. */
  function play(...steps: [Mode | 'rest', number][]) {
    timers.forEach(clearTimeout);
    timers = steps.map(([m, at]) => setTimeout(() => (mode = m === 'rest' ? rest : m), at));
  }

  function onEnter() {
    hovered = true;
    // Hovering its bed wakes the pet, and out it hops.
    if (pet.home) pet.wake('hover');
    // Choco keeps the island until it has answered; the cursor holds an answer up.
    if (choco.busy) {
      choco.hold(true);
      return;
    }
    if (dictation.state !== 'idle') return;
    // Back in during the leave grace: stay open.
    if (mode === 'expanded' || mode === 'settings' || mode === 'control') {
      play();
      return;
    }
    // The backend polls hover every 30ms, so these land close to Nusa's ~100ms / ~330ms.
    play(['peek', 50], ['expanded', 290]);
  }

  function onLeave() {
    hovered = false;
    choco.hold(false);
    // Typing into a field keeps the island open until the field is done.
    if (ui.typing || choco.busy || dictation.state !== 'idle') return;
    if (missed) {
      // Something happened while the island was open; show it now instead of collapsing.
      const p = missed;
      missed = null;
      showPopup(p);
    } else if (mode === 'expanded' || mode === 'settings' || mode === 'control')
      // A slip of the cursor shouldn't close it: hold a moment before collapsing.
      play(['peek', LEAVE_HOLD], ['rest', LEAVE_HOLD + 120]);
    else if (mode === 'popup') play(['peek', 0], ['rest', 120]);
    else play(['rest', 0]);
  }

  function showPopup(next: Popup) {
    if (hovered || ui.typing || choco.busy) {
      // Quick level changes aren't worth replaying; reminders are.
      if (next.hold) missed = next;
      return;
    }
    popup = next;
    // A reminder (one that holds) sends the pet running to the cursor.
    if (next.hold) pet.wake('nudge');
    const hold = next.hold ?? 1200;
    if (mode === 'popup') play(['popup', 0], ['peek', hold], ['rest', hold + 120]);
    else play(['peek', 0], ['popup', 100], ['peek', hold + 100], ['rest', hold + 220]);
  }

  function go(delta: number) {
    direction = delta;
    page = (page + delta + PAGES.length) % PAGES.length;
  }

  function goTo(i: number) {
    if (i === page) return;
    direction = i > page ? 1 : -1;
    page = i;
  }

  // Scroll down on the island for the Control Centre, up to come back; sideways turns pages.
  function onWheel(e: WheelEvent) {
    if (ui.typing || Date.now() - lastWheel < 350) return;
    const sideways = Math.abs(e.deltaX) > Math.abs(e.deltaY) || e.shiftKey;
    const d = sideways ? e.deltaX || e.deltaY : e.deltaY;
    if (d === 0) return;
    // A list that can still scroll that way keeps the wheel for itself.
    for (let el = e.target as HTMLElement | null; !sideways && el && el !== e.currentTarget; el = el.parentElement) {
      if (getComputedStyle(el).overflowY !== 'auto') continue;
      if (d < 0 ? el.scrollTop > 0 : el.scrollTop + el.clientHeight < el.scrollHeight - 1) return;
    }
    if (sideways) {
      if (mode !== 'expanded') return;
      lastWheel = Date.now();
      go(d > 0 ? 1 : -1);
    } else if (d > 0 && (mode === 'expanded' || mode === 'peek')) {
      lastWheel = Date.now();
      play(['control', 0]);
    } else if (d < 0 && mode === 'control') {
      lastWheel = Date.now();
      play(['expanded', 0]);
    }
  }

  // Tell the backend where the island is so clicks elsewhere pass through to the desktop.
  $effect(() => {
    invoke('set_hit_size', { width: shape.w, height: ISLAND_TOP + shape.h });
  });

  // The clock ticks twice a second while the island is open; resting, it only wakes on the
  // minute for the sleeping pill's clock.
  $effect(() => {
    clock.now = Date.now();
    if (mode === 'idle') {
      let id: ReturnType<typeof setTimeout>;
      const next = () => {
        id = setTimeout(() => {
          clock.now = Date.now();
          next();
        }, 60_000 - (Date.now() % 60_000) + 20);
      };
      next();
      return () => clearTimeout(id);
    }
    const id = setInterval(() => (clock.now = Date.now()), 500);
    return () => clearInterval(id);
  });

  // Once a field is done and the cursor has already gone, close as if it just left.
  $effect(() => {
    if (!ui.typing && !hovered && (mode === 'expanded' || mode === 'settings' || mode === 'control')) onLeave();
  });

  $effect(() => {
    const check = () => {
      todos.check();
      water.check();
    };
    check();
    const id = setInterval(check, REMINDER_EVERY);
    return () => clearInterval(id);
  });

  $effect(() => status.start());
  $effect(() => choco.start());
  $effect(() => pet.start());
  // The pet keeps quiet while you focus, sleeps at night, bobs to music and listens with you.
  $effect(() => {
    const calm = focus.running;
    const send = () => pet.context(calm, isNight());
    send();
    const id = setInterval(send, 60_000);
    return () => clearInterval(id);
  });
  $effect(() => {
    pet.mood({ music: !!nowPlaying.media?.playing, listening: choco.state === 'listening', calm: focus.running });
  });
  $effect(() => {
    document.documentElement.dataset.colour = ui.colour;
  });

  // "Hey Choco" takes the island over from whatever it was showing, and hands it back after.
  $effect(() => {
    const s = choco.state;
    untrack(() => {
      if (s === 'listening') pet.wake('voice');
      if (s === 'listening' || s === 'thinking') play(['voice', 0]);
      else if (s === 'working') play(['work', 0]);
      else if (s === 'answer') play(['answer', 0]);
      else if (voice) {
        if (hovered) play(['expanded', 0]);
        else if (missed) {
          const p = missed;
          missed = null;
          showPopup(p);
        } else play(['rest', 0]);
      }
    });
  });
  $effect(() => onPopup(showPopup));

  // Dictation shows on the island while it records and types, then hands it back.
  $effect(() => dictation.start());
  $effect(() => {
    const busy = dictation.state !== 'idle';
    untrack(() => {
      if (busy) play(['dictate', 0]);
      else if (mode === 'dictate') play([hovered ? 'expanded' : 'rest', 0]);
    });
  });

  // Files dragged over the island open the shelf; dropping parks them there.
  $effect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((e) => {
      const t = e.payload.type;
      shelf.hovering = t === 'enter' || t === 'over';
      if (t === 'enter' && page !== SHELF_PAGE) {
        direction = SHELF_PAGE > page ? 1 : -1;
        page = SHELF_PAGE;
      }
      if (t === 'drop') shelf.add(e.payload.paths);
    });
    return () => unlisten.then((off) => off());
  });

  // Music starting or stopping while the island is resting moves it between idle and the live bar.
  $effect(() => {
    if (!hovered && (mode === 'idle' || mode === 'live') && mode !== rest) mode = rest;
  });

  // Keep the music position (and so the current lyric) moving while it plays and can be seen.
  $effect(() => {
    if (mode === 'idle' || !nowPlaying.media?.playing) return;
    const id = setInterval(() => (nowPlaying.now = Date.now()), 200);
    return () => clearInterval(id);
  });

  $effect(() => nowPlaying.start());
  $effect(() => serveSettings());

  $effect(() => {
    const unlisten = [
      listen<boolean>('island-hover', (e) => (e.payload ? onEnter() : onLeave())),
      listen<boolean>('island-caps', (e) =>
        showPopup({ icon: 'caps', label: 'Caps Lock', value: e.payload ? 'On' : 'Off', tone: e.payload ? 'ok' : undefined }),
      ),
    ];
    // Catch up if the cursor was already over the island when this page loaded.
    invoke<boolean>('is_hovered').then((over) => over && onEnter());
    return () => unlisten.forEach((p) => p.then((off) => off()));
  });
</script>

<div
  class="island"
  class:grow
  class:idle={mode === 'idle'}
  style="--w: {shape.w}px; --h: {shape.h}px; --r: {shape.r}px"
  onwheel={onWheel}
>
  <!-- The resting live bar refreshes its glass at 10fps; anything you're interacting with gets 30fps. -->
  <!-- The resting pill barely changes, so its glass refreshes once a second; the live bar at 10fps;
       anything you're interacting with at 30fps. -->
  <Glass
    {shape}
    active
    frameMs={mode === 'idle' ? 1000 : mode === 'live' || mode === 'work' ? 100 : 33}
    prewarm={[AWAY, SLEEPING, SHAPES.peek, SHAPES.expanded, SHAPES.wide, SHAPES.live, SHAPES.popup, SHAPES.voice, SHAPES.work]}
  />
  <div class="glow" class:on={glow} style="--album: {glowColour ?? '255 255 255'}"></div>

  {#if mode === 'idle' && petOut}
    <!-- Its bed, empty while it's out. -->
  {:else if mode === 'idle'}
    <div class="layer pet" class:sleeping={idleMood === 'sleep'} in:materialise={{ duration: 200, delay: 100 }} out:fade={{ duration: 80 }}>
      {#if idleMood === 'sleep'}
        <span class="clock"><Digits value={time} /></span>
        <ChocoFace mood={idleMood} size={21} />
        <span class="day">{day}</span>
      {:else}
        <ChocoFace mood={idleMood} size={21} />
      {/if}
    </div>
  {:else if mode === 'peek'}
    <div class="layer peek" in:materialise={{ duration: 120 }} out:fade={{ duration: 80 }}>
      <BatteryRing />
      <span class="time">{time}</span>
      <Bars playing={!!nowPlaying.media?.playing} colour={nowPlaying.colour ? `rgb(${nowPlaying.colour})` : 'var(--text-dim)'} size={13} />
    </div>
  {:else if mode === 'expanded'}
    <div class="layer expanded" in:materialise={{ duration: 160, delay: 90 }} out:fade={{ duration: 80 }}>
      {#if wide}
        <div class="panel wide" in:fade={{ duration: 220, delay: 120 }} out:fade={{ duration: 120 }}>
          <LyricsView onLyrics={toggleLyrics} />
        </div>
      {:else}
        <div class="panel" in:fade={{ duration: 220, delay: 120 }} out:fade={{ duration: 120 }}>
          <div class="body">
            {#key page}
              <div
                class="page"
                in:fly={{ x: 40 * direction, duration: 300, easing: cubicOut }}
                out:fly={{ x: -40 * direction, duration: 200, easing: cubicOut }}
              >
                {#if page === 0}
                  <NowPlaying {lyricsOn} onLyrics={toggleLyrics} />
                {:else if page === 1}
                  <FocusPage />
                {:else if page === 2}
                  <WaterPage />
                {:else if page === 3}
                  <ShelfPage />
                {:else}
                  <TodoPage />
                {/if}
              </div>
            {/key}
          </div>

          <header>
            <!-- The music page shows its artwork where the name would be. -->
            {#if page !== 0}
              {#key page}
                <span class="title" in:fade={{ duration: 200 }}><i style="background: {current.accent}"></i>{current.name}</span>
              {/key}
            {/if}
            <span class="clock">{date}<b>{time}</b><BatteryRing /></span>
            <IconButton icon="settings" size={24} quiet label="Settings" onclick={() => play(['settings', 0])} />
          </header>

          <!-- Arrows stay hidden until the cursor nears an edge. -->
          <div class="edge left">
            <button class="nav" aria-label="Previous page" onclick={() => go(-1)}><Icon name="chevron-left" size={16} /></button>
          </div>
          <div class="edge right">
            <button class="nav" aria-label="Next page" onclick={() => go(1)}><Icon name="chevron-right" size={16} /></button>
          </div>

          <div class="dots">
            {#each PAGES as p, i}
              <button class="dot" class:active={i === page} aria-label={p.name} onclick={() => goTo(i)}><i></i></button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {:else if mode === 'live'}
    <div class="layer live" class:timer={liveTimer} in:materialise={{ duration: 160, delay: 80 }} out:fade={{ duration: 80 }}>
      <span class="note-icon" class:timer={liveTimer}>
        {#if liveTimer}<Icon name="timer" size={15} />{:else}<Bars playing colour={glowColour ? `rgb(${glowColour})` : 'var(--text)'} size={13} />{/if}
      </span>
      {#key liveText}
        <span class="line" in:fade={{ duration: 250 }}>{liveText}</span>
      {/key}
      {#if liveTimer}
        <b class="countdown">{mmss(Math.ceil(focus.remaining(clock.now) / 1000) * 1000)}</b>
      {/if}
    </div>
  {:else if mode === 'control'}
    <div class="layer" bind:offsetHeight={controlH} in:materialise={{ duration: 200, delay: 90 }} out:fade={{ duration: 80 }}>
      <ControlCentre />
    </div>
  {:else if mode === 'settings'}
    <div class="layer" bind:offsetHeight={settingsH} in:materialise={{ duration: 180, delay: 100 }} out:fade={{ duration: 80 }}>
      <Settings {lyricsOn} onLyrics={toggleLyrics} onclose={() => play(['expanded', 0])} />
    </div>
  {:else if mode === 'voice'}
    {@const thinking = choco.state === 'thinking'}
    <div class="layer voice" in:materialise={{ duration: 140, delay: 60 }} out:fade={{ duration: 80 }}>
      <ChocoFace mood={thinking ? 'think' : 'alert'} size={17} level={thinking ? 0 : choco.level} />
      <Wave level={choco.level} {thinking} bars={18} />
      {#key choco.heard}
        <span class="said" class:waiting={!choco.heard} in:fade={{ duration: 220 }}>
          {choco.heard || (thinking ? '' : 'Listening')}
        </span>
      {/key}
    </div>
  {:else if mode === 'dictate'}
    {@const thinking = dictation.state === 'thinking'}
    <div class="layer voice" in:materialise={{ duration: 140, delay: 60 }} out:fade={{ duration: 80 }}>
      <span class="mic" class:failed={dictation.failed}><Icon name="mic" size={17} /></span>
      <Wave level={dictation.level} {thinking} bars={18} />
      {#key dictation.text}
        <span class="said" class:waiting={!dictation.text} in:fade={{ duration: 220 }}>
          {dictation.text || (thinking ? 'Typing' : 'Dictating')}
        </span>
      {/key}
    </div>
  {:else if mode === 'work'}
    <!-- Working the desktop: what it's doing, or a Yes / No before anything risky. -->
    <div class="layer work" in:materialise={{ duration: 140, delay: 60 }} out:fade={{ duration: 80 }}>
      <ChocoFace mood={choco.question ? 'alert' : 'think'} size={17} />
      {#key choco.question ?? choco.step}
        <span class="said" class:asking={choco.question} in:fade={{ duration: 220 }}>{choco.question ?? choco.step}</span>
      {/key}
      {#if choco.question}
        <Pill onclick={() => choco.answerQuestion(false)}>No</Pill>
        <Pill primary onclick={() => choco.answerQuestion(true)}>Yes</Pill>
      {:else}
        <button class="stop" onclick={() => choco.stopWork()}>Esc to stop</button>
      {/if}
    </div>
  {:else if mode === 'answer' && choco.reply}
    <div class="layer" bind:offsetHeight={answerH} in:materialise={{ duration: 160, delay: 110 }} out:fade={{ duration: 100 }}>
      {#key choco.reply}
        <Answer
          reply={choco.reply}
          heard={choco.heard}
          followUp={choco.followUp}
          level={choco.level}
          width={answerW}
          onrevealed={() => choco.revealed()}
        />
      {/key}
    </div>
  {:else if mode === 'popup'}
    <div class="layer popup" in:materialise={{ duration: 120, delay: 60 }} out:fade={{ duration: 80 }}>
      {#key popup}
        <span class="popup-icon" style="--accent: {popup.accent ?? 'var(--text)'}"><Icon name={popup.icon} size={15} /></span>
      {/key}
      <span class="label" class:grow={popup.level === undefined}>{popup.label}</span>
      {#if popup.level !== undefined}
        <span class="level"><i style="width: {popup.level}%"></i></span>
      {/if}
      {#if popup.value}
        <span class="value" class:ok={popup.tone === 'ok'} class:bad={popup.tone === 'bad'}>{popup.value}</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .island {
    position: absolute;
    top: var(--island-top);
    left: 50%;
    transform: translateX(-50%);
    width: var(--w);
    height: var(--h);
    border-radius: var(--r);
    /* A tight contact shadow and a wide soft one lift it off the desktop. */
    box-shadow:
      0 1px 2px rgba(0, 0, 0, calc(var(--glass-shadow) * 0.5)),
      0 6px 14px rgba(0, 0, 0, calc(var(--glass-shadow) * 0.35)),
      0 18px 50px rgba(0, 0, 0, calc(var(--glass-shadow) * 0.7));
    overflow: hidden;
    isolation: isolate;
    transition-property: width, height, border-radius, box-shadow;
    transition-duration: var(--morph-shrink);
    transition-timing-function: var(--ease-out);
  }
  .island.grow {
    transition-duration: var(--morph-grow);
    transition-timing-function: var(--spring-morph);
  }
  /* The resting pill is small; a deep shadow under it would look like a smudge. */
  .island.idle {
    box-shadow:
      0 1px 2px rgba(0, 0, 0, calc(var(--glass-shadow) * 0.5)),
      0 4px 10px rgba(0, 0, 0, calc(var(--glass-shadow) * 0.3));
  }

  /* Each state's content keeps its final size and is clipped by the morphing shape,
     so nothing stretches mid-animation. */
  .layer {
    position: absolute;
    top: 0;
    left: 50%;
    transform: translateX(-50%);
  }

  .peek {
    width: 190px;
    height: 28px;
    padding: 0 14px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .peek .time {
    font-size: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.2px;
  }

  /* Normal and wide (lyrics) layouts cross-fade inside one layer, each centred at its own width. */
  .expanded {
    width: 0;
    height: 180px;
  }
  .panel {
    position: absolute;
    top: 0;
    left: 50%;
    transform: translateX(-50%);
    width: 545px;
    height: 180px;
  }
  .panel.wide {
    width: 675px;
  }
  header {
    position: absolute;
    top: 12px;
    left: 28px;
    right: 26px;
    height: 20px;
    display: flex;
    align-items: center;
    font-size: 12.5px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 7px;
    font-weight: 600;
    color: var(--text-dim);
  }
  .title i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    box-shadow: 0 0 8px currentColor;
  }
  .clock {
    margin-left: auto;
    margin-right: 2px;
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .clock b {
    margin-right: 4px;
    color: var(--text);
    font-weight: 600;
  }

  .edge {
    position: absolute;
    top: 44px;
    bottom: 28px;
    width: 36px;
    display: grid;
    place-items: center;
  }
  .edge.left {
    left: 0;
  }
  .edge.right {
    right: 0;
  }
  .nav {
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 50%;
    padding: 0;
    display: grid;
    place-items: center;
    background: var(--pill);
    color: var(--text);
    cursor: pointer;
    opacity: 0;
    transform: scale(0.9);
    transition:
      opacity 180ms,
      transform 260ms var(--spring),
      background 160ms;
  }
  .edge:hover .nav {
    opacity: 1;
    transform: scale(1);
  }
  .nav:hover {
    background: var(--pill-hover);
  }
  .nav:active {
    transform: scale(0.96);
  }

  .body,
  .page {
    position: absolute;
    inset: 0;
  }

  .dots {
    position: absolute;
    bottom: 8px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
  }
  /* Each dot sits in a bigger hit area so it is easy to click. */
  .dot {
    height: 14px;
    padding: 0 3px;
    border: none;
    background: none;
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  .dot i {
    width: 5px;
    height: 5px;
    border-radius: 3px;
    background: var(--dot);
    transition:
      width 320ms var(--spring),
      background 250ms;
  }
  .dot:hover i {
    background: var(--text-dim);
  }
  .dot.active i {
    width: 16px;
    background: var(--text);
  }

  .popup {
    width: 425px;
    height: 36px;
    padding: 0 16px 0 8px;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .popup-icon {
    width: 24px;
    height: 24px;
    flex: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    animation: pop 320ms var(--spring);
  }
  @keyframes pop {
    from {
      transform: scale(0.9);
      opacity: 0;
    }
  }
  .popup .label {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .popup .label.grow {
    flex: 1;
  }
  .level {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: var(--track);
    overflow: hidden;
  }
  .level i {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--text);
    box-shadow: 0 0 8px rgba(255, 255, 255, 0.6);
    transition: width 160ms var(--ease-out);
  }
  .popup .value {
    flex: none;
    min-width: 24px;
    text-align: right;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .popup .value.ok {
    color: var(--ok);
  }
  .popup .value.bad {
    color: var(--bad);
  }

  /* Album-colour light rising from the bottom edge, in lyrics view and the live bar. */
  .glow {
    position: absolute;
    inset: 0;
    background: linear-gradient(to top, rgb(var(--album) / 0.5), rgb(var(--album) / 0.12) 55%, transparent 85%);
    opacity: 0;
    transition: opacity 400ms var(--ease-out);
  }
  .glow.on {
    opacity: 1;
  }
  /* Choco, resting. */
  .pet {
    width: 100px;
    height: 30px;
    display: grid;
    place-items: center;
    padding-top: 1px;
  }
  .pet.sleeping {
    width: 184px;
    height: 32px;
    grid-template-columns: 1fr auto 1fr;
    gap: 12px;
    padding: 1px 16px 0;
  }
  .clock,
  .day {
    font-size: 13px;
    letter-spacing: -0.1px;
    animation: wake-in 600ms var(--ease-out) both;
  }
  .clock {
    justify-self: start;
    font-weight: 600;
    color: var(--text);
  }
  .day {
    justify-self: end;
    font-weight: 500;
    color: var(--text-dim);
    animation-delay: 60ms;
  }
  /* The words surface out of the glass as the pill settles. */
  @keyframes wake-in {
    from {
      opacity: 0;
      filter: blur(4px);
      transform: scale(0.94);
    }
  }

  /* Listening: Choco's ears up and your voice as a soft wave. */
  .voice {
    width: 320px;
    height: 44px;
    padding: 0 18px 0 16px;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .mic {
    display: grid;
    color: var(--accent-focus);
  }
  .mic.failed {
    color: var(--bad);
  }
  .said {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    font-weight: 500;
    text-align: right;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .said.waiting {
    color: var(--text-faint);
  }
  .work {
    width: 440px;
    height: 44px;
    padding: 0 8px 0 16px;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .work .said {
    text-align: left;
    color: var(--text-dim);
  }
  .work .said.asking {
    color: var(--text);
  }
  .stop {
    flex: none;
    height: 28px;
    padding: 0 10px;
    border: none;
    border-radius: 14px;
    background: none;
    color: var(--text-faint);
    font: 500 12px var(--font);
    cursor: pointer;
  }
  .stop:hover {
    color: var(--text);
  }

  .live {
    width: 390px;
    height: 36px;
    padding: 0 18px;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .note-icon {
    width: 40px;
    flex: none;
    display: inline-flex;
    color: var(--text);
  }
  .live .countdown,
  .live .line {
    letter-spacing: 0.1px;
  }
  .note-icon.timer {
    color: var(--accent-focus);
  }
  .countdown {
    width: 40px;
    flex: none;
    text-align: right;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .live .line {
    flex: 1;
    min-width: 0;
    text-align: center;
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Balance the icon on the left so the line sits in the true centre. */
  .live:not(.timer)::after {
    content: '';
    width: 40px;
    flex: none;
  }
</style>
