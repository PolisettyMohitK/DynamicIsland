<script lang="ts">
  // The pet window: Choco out on the desktop. Rust moves the window and says how it's moving;
  // this page picks the pose and the little tricks. Nothing loops while it sits still.
  //
  // Resting goes stand → sit → the odd trick → (a yawn) → lie down, like a dog settling in.
  // Tricks are weighted by energy and never repeat back to back.
  //
  // Click it and it stays: sits right there watching the cursor, and if it's left long enough,
  // curls up and dozes off. Click again and it's up with a woof and following.
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
  /** Staying this long and it falls asleep where it sits. */
  const STAY_SLEEP = 30_000;
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
  let gaze = $state(0);
  let staying = $state(false);
  let asleep = $state(false);
  let pose = $state<Pose>('stand');
  let trick = $state<Act>('none');
  let petted = $state(false);
  let bark = $state(0);
  let config = $state<Config>({ enabled: true, energy: 'calm', sounds: false });
  let mood = $state<Mood>({ music: false, listening: false, calm: false });

  /** What the dog shows: moving beats everything, then sleep, petting, staying and listening,
   *  then a trick. */
  const act = $derived.by((): Act => {
    if (motion !== 'rest') return motion;
    if (asleep) return 'sleep';
    if (petted) return 'happy';
    if (staying) return trick === 'none' ? 'watch' : trick;
    if (mood.listening) return 'listen';
    return trick;
  });

  $effect(() => {
    invoke<Config>('pet_config').then((c) => (config = c));
    const offs = [
      listen<{ motion: Motion; facing: number; slope: number; gaze: number }>('pet-motion', (e) => {
        // Pulling up from a run: a little skid.
        if (e.payload.motion === 'rest' && motion === 'run') play('skid');
        // Off again (a reminder, sent home, a new trip out): no longer staying.
        if (e.payload.motion !== 'rest') staying = asleep = false;
        motion = e.payload.motion;
        facing = e.payload.facing;
        slope = e.payload.slope;
        gaze = e.payload.gaze;
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
    if (staying) {
      // Sits at once and watches; no tricks, just a yawn before it nods off.
      pose = 'sit';
      later(STAY_SLEEP - 1700, () => play('yawn'));
      later(STAY_SLEEP, () => {
        pose = 'lie';
        asleep = true;
      });
      return () => timers.forEach(clearTimeout);
    }
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
    if (!mood.music || motion !== 'rest' || mood.calm || staying) return;
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

  let clickTimer: ReturnType<typeof setTimeout> | undefined;
  function click() {
    // Wait a moment so a double-click (go home) doesn't count as a click too.
    clearTimeout(clickTimer);
    clickTimer = setTimeout(() => stay(!staying), 250);
  }
  /** Sit and stay, or (already staying, even asleep) up with a woof and follow again. */
  function stay(on: boolean) {
    staying = on;
    invoke('pet_stay', { stay: on });
    if (on) return;
    asleep = false;
    pose = 'stand';
    woof();
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
    <Dog {pose} {act} {slope} {gaze} />
  </div>
  {#if act === 'sleep'}
    <!-- Outside the flipped dog so the z's read the right way round. -->
    <span class="zzz" style="--side: {facing}"><i>z</i><i>z</i><i>z</i></span>
  {/if}
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
  /* Asleep: little z's drifting up off its head, one after another, growing as they go. */
  .zzz {
    position: absolute;
    left: calc(50% + var(--side) * 20px);
    bottom: 34px;
    pointer-events: none;
  }
  .zzz i {
    position: absolute;
    left: 0;
    bottom: 0;
    color: #f7f7f9;
    font-size: 10px;
    font-style: normal;
    font-weight: 700;
    -webkit-text-stroke: 0.8px #2a2a30;
    opacity: 0;
    animation: zzz 2.7s ease-out infinite;
  }
  .zzz i:nth-child(2) {
    animation-delay: 0.9s;
  }
  .zzz i:nth-child(3) {
    animation-delay: 1.8s;
  }
  @keyframes zzz {
    0% {
      opacity: 0;
      transform: translate(0, 0) scale(0.6);
    }
    20% {
      opacity: 1;
    }
    100% {
      opacity: 0;
      transform: translate(calc(var(--side) * 10px), -26px) scale(1.3);
    }
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
