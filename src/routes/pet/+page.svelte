<script lang="ts">
  // The pet window: Choco out on the desktop. Rust moves the window and says how it's moving;
  // this page picks the pose and the little tricks. Nothing loops while it sits still.
  //
  // Resting goes stand → sit → the odd trick → (a yawn) → lie down, like a dog settling in.
  // Tricks are weighted by energy and never repeat back to back.
  import '$lib/theme.css';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Dog, { type Act, type Pose } from '$lib/components/Dog.svelte';

  type Motion = 'walk' | 'run' | 'rest' | 'alert' | 'wake' | 'climb' | 'nudge';
  type Energy = 'calm' | 'playful';
  type Config = { enabled: boolean; energy: Energy; sounds: boolean };
  /** From the island: what's going on around it. */
  type Mood = { music: boolean; listening: boolean; calm: boolean };

  const SIT_AFTER = 1200;
  const LIE_AFTER: Record<Energy, number> = { calm: 35_000, playful: 70_000 };
  const PET_AFTER = 1000;
  const BOB_EVERY = 5000;
  /** Seconds between tricks: [min, max]. */
  const TRICK_GAP: Record<Energy, [number, number]> = { calm: [12, 20], playful: [6, 11] };
  /** Which tricks, how likely, by energy and pose. */
  const TRICKS: Record<Energy, Partial<Record<Act, number>>> = {
    calm: { yawn: 3, sniff: 2, scratch: 2, look: 3, wag: 1 },
    playful: { wag: 3, look: 3, hop: 2, sniff: 2, scratch: 1, yawn: 1 },
  };
  const LYING: Act[] = ['look', 'yawn', 'sniff', 'wag'];
  /** How long each act plays before it settles back. */
  const ACT_MS: Partial<Record<Act, number>> = {
    scratch: 1500,
    yawn: 1700,
    wag: 2500,
    sniff: 1900,
    look: 2600,
    bob: 1850,
    hop: 600,
    bark: 700,
    skid: 400,
  };

  let motion = $state<Motion>('rest');
  let facing = $state(1);
  let slope = $state(0);
  let pose = $state<Pose>('stand');
  let trick = $state<Act>('none');
  let petted = $state(false);
  let bark = $state(0);
  let config = $state<Config>({ enabled: true, energy: 'calm', sounds: false });
  let mood = $state<Mood>({ music: false, listening: false, calm: false });

  /** What the dog shows: moving beats everything, then petting and listening, then a trick. */
  const act = $derived.by((): Act => {
    if (motion !== 'rest') return motion;
    if (petted) return 'happy';
    if (mood.listening) return 'listen';
    return trick;
  });

  $effect(() => {
    invoke<Config>('pet_config').then((c) => (config = c));
    const offs = [
      listen<{ motion: Motion; facing: number; slope: number }>('pet-motion', (e) => {
        // Pulling up from a run: a little skid.
        if (e.payload.motion === 'rest' && motion === 'run') play('skid');
        motion = e.payload.motion;
        facing = e.payload.facing;
        slope = e.payload.slope;
      }),
      listen<Config>('pet-config', (e) => (config = e.payload)),
      listen<Mood>('pet-mood', (e) => (mood = e.payload)),
      listen<boolean>('pet-hover', (e) => hover(e.payload)),
    ];
    // Rust shows the window (and wakes the dog) once we're listening.
    Promise.all(offs).then(() => invoke('pet_ready'));
    return () => offs.forEach((o) => o.then((f) => f()));
  });

  // Resting: settle in step by step. Focusing: lie down quietly at once, no tricks.
  $effect(() => {
    if (motion !== 'rest') {
      pose = 'stand';
      return;
    }
    const energy = config.energy;
    const timers: ReturnType<typeof setTimeout>[] = [];
    const later = (ms: number, f: () => void) => timers.push(setTimeout(f, ms));
    if (mood.calm) {
      later(400, () => (pose = 'lie'));
      return () => timers.forEach(clearTimeout);
    }
    later(SIT_AFTER, () => (pose = 'sit'));
    later(LIE_AFTER[energy] - 1700, () => play('yawn'));
    later(LIE_AFTER[energy], () => (pose = 'lie'));
    let last: Act = 'none';
    const next = () => {
      const [lo, hi] = TRICK_GAP[energy];
      later((lo + Math.random() * (hi - lo)) * 1000, () => {
        if (trick === 'none' && !petted) last = play(pick(energy, last));
        next();
      });
    };
    next();
    return () => timers.forEach(clearTimeout);
  });

  // Music: a little head bob now and then while it rests.
  $effect(() => {
    if (!mood.music || motion !== 'rest' || mood.calm) return;
    const id = setInterval(() => trick === 'none' && play('bob'), BOB_EVERY);
    return () => clearInterval(id);
  });

  /** A weighted pick that suits the pose, never the same twice running. */
  function pick(energy: Energy, last: Act): Act {
    const fits = (t: Act) => t !== last && (pose === 'lie' ? LYING.includes(t) : t !== 'scratch' || pose === 'sit');
    const options = Object.entries(TRICKS[energy]).filter(([t]) => fits(t as Act)) as [Act, number][];
    let r = Math.random() * options.reduce((n, [, w]) => n + w, 0);
    for (const [t, w] of options) if ((r -= w) < 0) return t;
    return 'look';
  }

  let trickTimer: ReturnType<typeof setTimeout> | undefined;
  function play(t: Act): Act {
    clearTimeout(trickTimer);
    // The same act again restarts it: clear for a frame first.
    if (trick === t) {
      trick = 'none';
      requestAnimationFrame(() => (trick = t));
    } else trick = t;
    trickTimer = setTimeout(() => (trick = 'none'), ACT_MS[t] ?? 1500);
    return t;
  }

  let petTimer: ReturnType<typeof setTimeout> | undefined;
  function hover(on: boolean) {
    clearTimeout(petTimer);
    if (on) petTimer = setTimeout(() => (petted = true), PET_AFTER);
    else petted = false;
  }

  let clicks = 0;
  let clickTimer: ReturnType<typeof setTimeout> | undefined;
  function click() {
    // Wait a moment so a double-click (go home) doesn't bark first. Hop and bark take turns.
    clearTimeout(clickTimer);
    clickTimer = setTimeout(() => (clicks++ % 2 ? play('hop') : woof()), 250);
  }
  function home() {
    clearTimeout(clickTimer);
    invoke('pet_home');
  }

  function woof() {
    play('bark');
    bark++;
    if (config.sounds) yip();
  }

  /** A soft, short yip made on the spot: two quick falling tones. */
  function yip() {
    const ctx = new AudioContext();
    const t = ctx.currentTime;
    for (const at of [0, 0.16]) {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = 'triangle';
      osc.frequency.setValueAtTime(620, t + at);
      osc.frequency.exponentialRampToValueAtTime(330, t + at + 0.1);
      gain.gain.setValueAtTime(0.0001, t + at);
      gain.gain.exponentialRampToValueAtTime(0.12, t + at + 0.015);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + at + 0.12);
      osc.connect(gain).connect(ctx.destination);
      osc.start(t + at);
      osc.stop(t + at + 0.13);
    }
    setTimeout(() => ctx.close(), 500);
  }
</script>

<main>
  {#key bark}
    {#if bark}<span class="woof" style="--side: {facing}">woof</span>{/if}
  {/key}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="dog" style="transform: scaleX({facing})" onclick={click} ondblclick={home}>
    <Dog {pose} {act} {slope} />
  </div>
</main>

<style>
  main {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    user-select: none;
    cursor: default;
  }
  /* Turning round: a quick squeeze through the middle rather than an instant flip. */
  .dog {
    width: 70px;
    height: 54px;
    transition: transform 160ms var(--ease-out);
  }
  /* A small speech bubble over its head; never flipped, so it reads. */
  .woof {
    position: absolute;
    left: calc(50% + var(--side) * 18px);
    bottom: 58px;
    transform: translateX(-50%);
    padding: 2px 8px;
    border-radius: 10px;
    background: #f7f7f9;
    color: #1d1d22;
    box-shadow: 0 0 0 1.2px #2a2a30;
    font-size: 11px;
    font-weight: 600;
    animation: woof 900ms var(--ease-out) forwards;
  }
  @keyframes woof {
    0% {
      opacity: 0;
      transform: translate(-50%, 4px) scale(0.8);
    }
    20% {
      opacity: 1;
      transform: translate(-50%, 0);
    }
    75% {
      opacity: 1;
    }
    100% {
      opacity: 0;
      transform: translate(-50%, -4px);
    }
  }
</style>
