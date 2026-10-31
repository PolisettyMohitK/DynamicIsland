<script lang="ts" module>
  export type Mood = 'idle' | 'sleep' | 'alert' | 'think' | 'happy' | 'sad' | 'vibe';
</script>

<script lang="ts">
  // Choco: a small floppy-eared dog that lives in the notch. Its mood shows what the island is doing.
  let { mood = 'idle', size = 18, level = 0 }: { mood?: Mood; size?: number; level?: number } = $props();

  const EAR = 'M11 4.6C7.2 4 3.6 6.8 3.3 12.2c-.2 3.2 2.9 3.8 4.3 1.3L10.4 9';

  // Resting moods move in short bursts with still pauses between: a never-ending animation
  // would repaint the glass every frame. Each beat replays the mood's animation once.
  const BURST_MS: Partial<Record<Mood, number>> = { idle: 6000, sleep: 8000, vibe: 10_000 };
  let beat = $state(0);
  $effect(() => {
    const every = BURST_MS[mood];
    if (!every) return;
    const id = setInterval(() => beat++, every);
    return () => clearInterval(id);
  });
</script>

{#key beat}
  <!-- Whole-face moves (breath, bob) and the z live on HTML boxes, which the GPU moves on its
       own; moving shapes inside the SVG would lay it out again every frame. -->
  <span class="choco {mood}" style="--size: {size}px">
  <svg
    class="face {mood}"
    class:alt={beat % 2 === 1}
    viewBox="0 0 40 24"
    style="height: {size}px; width: {(size * 40) / 24}px; --lvl: {level}"
    aria-hidden="true"
  >
    <g class="head">
      <g class="ear left"><path d={EAR} /></g>
      <g transform="matrix(-1 0 0 1 40 0)"><g class="ear right"><path d={EAR} /></g></g>

      {#if mood === 'sleep'}
        <path class="lid" d="M13.2 11.6q1.8 1.3 3.6 0M23.2 11.6q1.8 1.3 3.6 0" />
      {:else if mood === 'happy'}
        <path class="lid" d="M13.2 12.2q1.8-2 3.6 0M23.2 12.2q1.8-2 3.6 0" />
      {:else}
        <g class="eyes">
          <ellipse cx="15" cy="11" rx="1.75" ry="1.95" />
          <ellipse cx="25" cy="11" rx="1.75" ry="1.95" />
        </g>
      {/if}

      <ellipse class="nose" cx="20" cy="15.3" rx="1.9" ry="1.3" />
      <path class="mouth" d="M18.1 17.5q.95.9 1.9 0q.95.9 1.9 0" />
    </g>
  </svg>
  {#if mood === 'sleep'}<span class="z">z</span>{/if}
  </span>
{/key}

<style>
  .choco {
    position: relative;
    display: block;
    flex: none;
  }
  .face {
    display: block;
    flex: none;
    overflow: visible;
    color: var(--text);
  }
  path,
  ellipse {
    fill: currentColor;
  }
  .ear path {
    opacity: 0.85;
  }
  .lid,
  .mouth {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.2;
    stroke-linecap: round;
  }
  .mouth {
    stroke-width: 0.9;
    opacity: 0.7;
  }
  .ear {
    transform-origin: 10.5px 5px;
    transition: transform 420ms var(--spring);
  }
  .eyes {
    transform-origin: 20px 11px;
    transform-box: view-box;
    transition: transform 300ms var(--ease-out);
  }
  .head {
    transform-box: view-box;
    transform-origin: 20px 14px;
  }

  /* Idle: blinks now and then, and every other time glances around. */
  .idle .eyes {
    animation: blink 300ms;
  }
  .idle.alt .eyes {
    animation:
      blink 300ms,
      glance 1.8s ease-in-out;
  }
  @keyframes blink {
    50% {
      transform: scaleY(0.1);
    }
  }
  @keyframes glance {
    20%,
    45% {
      translate: 1px 0;
    }
    60%,
    85% {
      translate: -1px 0;
    }
  }

  /* Asleep: a slow breath and a drifting z. */
  .choco.sleep {
    transform-origin: 50% 58%;
    animation: breathe 3s ease-in-out;
  }
  .sleep .ear {
    transform: rotate(-6deg);
  }
  @keyframes breathe {
    50% {
      transform: translateY(0.6px) scale(0.98);
    }
  }
  /* Where the SVG's z sat: x 33, baseline 7 of the 40×24 face. */
  .z {
    position: absolute;
    left: 82.5%;
    top: 0;
    color: var(--text);
    font: 600 calc(var(--size) / 4) / 1 var(--font);
    pointer-events: none;
    opacity: 0;
    animation: z 3s ease-out;
  }
  @keyframes z {
    20% {
      opacity: 0.7;
    }
    100% {
      opacity: 0;
      transform: translate(3px, -5px);
    }
  }

  /* Listening: ears up, eyes wide; the ears twitch with your voice. */
  .alert .ear {
    transform: rotate(calc(24deg + var(--lvl) * 14deg));
    transition-duration: 140ms;
  }
  .alert .eyes {
    transform: scale(1.15);
  }

  /* Thinking: looks up and around. */
  .think .ear {
    transform: rotate(10deg);
  }
  .think .eyes {
    animation: ponder 1.6s ease-in-out infinite;
  }
  @keyframes ponder {
    0%,
    100% {
      transform: translate(-1.2px, -1.2px);
    }
    50% {
      transform: translate(1.2px, -1.2px);
    }
  }

  .happy .ear {
    animation: wiggle 900ms var(--ease-out);
  }
  @keyframes wiggle {
    25% {
      transform: rotate(22deg);
    }
    50% {
      transform: rotate(4deg);
    }
    75% {
      transform: rotate(14deg);
    }
  }

  .sad .ear {
    transform: rotate(-14deg);
  }
  .sad .eyes {
    transform: translateY(0.8px) scaleY(0.75);
  }

  /* Music: bobs along for a few beats. */
  .choco.vibe {
    transform-origin: 50% 58%;
    animation: bob 0.9s ease-in-out 2;
  }
  .vibe .ear {
    animation: flop 0.9s ease-in-out 2;
  }
  @keyframes bob {
    50% {
      transform: translateY(1px) rotate(-3deg);
    }
  }
  @keyframes flop {
    50% {
      transform: rotate(9deg);
    }
  }
</style>
