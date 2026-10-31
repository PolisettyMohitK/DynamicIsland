<script lang="ts" module>
  /** The body's shape. */
  export type Pose = 'stand' | 'sit' | 'lie';
  /** What it's doing on top of that. Most play once and end, so a still dog costs nothing. */
  export type Act =
    | 'none'
    | 'walk'
    | 'run'
    | 'alert'
    | 'skid'
    | 'wake'
    | 'climb'
    | 'happy'
    | 'wag'
    | 'yawn'
    | 'scratch'
    | 'sniff'
    | 'look'
    | 'listen'
    | 'bob'
    | 'hop'
    | 'nudge'
    | 'bark';
</script>

<script lang="ts">
  // Choco, full body, side on: a chibi dog in the same line-art as the face in the island
  // (floppy ear, dot eye, small nose). Light fur with a dark outline so it reads on any
  // wallpaper. Faces right; the page flips it.
  // slope: how far it leans while moving, in degrees; negative climbs up the screen, positive
  // heads down it. Steep enough and the stride changes too.
  let { pose, act, slope = 0 }: { pose: Pose; act: Act; slope?: number } = $props();

  // Head, drawn once and moved per pose: round skull, a short muzzle with a little stop.
  const HEAD = 'M50 7C44 7 40 11.5 40 17s4 10 10 10c3.5 0 6-.8 9-1.4 3.7-.8 6-2.6 6-4.8 0-2.6-2.2-4-5-4.2C59.3 11.6 55.6 7 50 7Z';
  const EAR = 'M46.5 9.3c-4.2-.4-6.3 3.4-5.2 9.5.5 3 3.6 3.8 4.8 1.2l2.8-7.4c.6-1.6-.4-3.1-2.4-3.3Z';
  const NECK = '46px 25px';
  const HEAD_AT: Record<Pose, string> = { stand: '', sit: 'translate(-6 -5)', lie: 'translate(2 15)' };

  const closedEyes = $derived(act === 'happy' || act === 'yawn' || act === 'scratch');
  const openMouth = $derived(act === 'yawn' || act === 'bark' || act === 'run' || act === 'happy');
</script>

{#snippet leg(x1: number, y1: number, x2: number, y2: number, cls = '')}
  <g class="leg {cls}" style="transform-origin: {x1}px {y1}px">
    <line class="out" {x1} {y1} {x2} {y2} />
    <line class="in" {x1} {y1} {x2} {y2} />
  </g>
{/snippet}

{#snippet tail(d: string, origin: string)}
  <g class="tail" style="transform-origin: {origin}">
    <path class="out" {d} />
    <path class="in" {d} />
  </g>
{/snippet}

<svg
  class="dog {pose} {act}"
  class:up={slope <= -12}
  class:down={slope >= 12}
  style="--slope: {slope}deg"
  viewBox="0 0 70 54"
  aria-hidden="true"
>
  <ellipse class="shadow" cx="34" cy="50.6" rx={pose === 'lie' ? 22 : pose === 'sit' ? 15 : 17} ry="2.2" />
  <g class="tilt">
  {#key pose}
    <g class="settle">
      <g class="whole">
        {#if pose === 'stand'}
          {@render tail('M15.5 27q-6-2-6.5-9.5', '15.5px 27px')}
          {@render leg(24, 31, 24, 47.6, 'far back b')}
          {@render leg(39.5, 31, 39.5, 47.6, 'far front a')}
          {@render leg(19.5, 31, 19.5, 47.6, 'back a')}
          {@render leg(43.5, 31, 43.5, 47.6, 'front b')}
          <path class="fur" d="M18 24.5C25 22 35 21.5 42 22.5c5 .7 7 4.5 6 8.5-.8 3.6-3.5 5.4-8 5.6-7 .4-14 .4-20 0-5-.4-7-3.6-6.6-6.6.4-3 2-4.6 4.6-5.5Z" />
        {:else if pose === 'sit'}
          {@render tail('M19 47q-6 2-11.5 .8', '19px 47px')}
          {@render leg(37, 31, 37, 47.6, 'far a')}
          {@render leg(41, 31, 41, 47.6, 'b')}
          <path class="fur" d="M36 21.5c5.6 0 8.6 6.2 7.8 14-.8 7.6-3.8 13-11.6 13.4-9.4.4-15-2.6-14.2-8.6.8-6.6 9.4-18.8 18-18.8Z" />
          {@render leg(28, 48, 34.5, 48, 'paw')}
          <ellipse class="fur haunch" cx="25.5" cy="41.5" rx="8.2" ry="7.2" />
        {:else}
          {@render tail('M13 44q-6 1.6-10 5', '13px 44px')}
          {@render leg(45, 46.4, 58, 46.4, 'far')}
          <path class="fur" d="M15 37.5c7-3.4 23.6-4 30.6-1.2 4.6 1.8 4.8 10.6.4 12.2-9 2.4-21.4 2.4-30.6.4-5.4-1.6-5.8-9.2-.4-11.4Z" />
          {@render leg(42.5, 48, 56.5, 48)}
          {@render leg(24, 48.2, 31, 48.2, 'paw')}
          <ellipse class="fur haunch" cx="21.5" cy="43.6" rx="7.4" ry="4.8" />
        {/if}

        <g transform={HEAD_AT[pose]}>
          <g class="head" style="transform-origin: {NECK}">
            <path class="fur" d={HEAD} />
            <g class="ear" style="transform-origin: 47px 10px"><path d={EAR} /></g>
            {#if closedEyes}
              <path class="line" d="M52.6 15.4q1.4-1.5 2.8 0" />
            {:else}
              <ellipse class="ink eye" cx="54" cy="15.2" rx="1.3" ry="1.55" />
            {/if}
            <ellipse class="ink nose" cx="64" cy="19.4" rx="1.75" ry="1.35" />
            {#if openMouth}
              <path class="ink mouth" d="M58.6 23.4q2.3 2.9 4.6 .1Z" />
            {:else}
              <path class="line" d="M59.4 23.2q1.4 1 2.8 0" />
            {/if}
          </g>
        </g>

        {#if pose === 'sit' && act === 'scratch'}
          <!-- The hind paw up at the ear. -->
          {@render leg(28, 40, 38, 25, 'scratcher')}
        {/if}
      </g>
    </g>
  {/key}
  </g>
</svg>

<style>
  .dog {
    display: block;
    width: 70px;
    height: 54px;
    overflow: visible;
  }
  .dog * {
    transform-box: view-box;
  }

  /* Light fur, dark outline; the far legs a shade darker for depth. */
  .fur,
  .ear path {
    fill: #f7f7f9;
    stroke: #2a2a30;
    stroke-width: 1.4;
    stroke-linejoin: round;
  }
  .ear path {
    fill: #d9d9df;
  }
  .leg line,
  .tail path {
    fill: none;
    stroke-linecap: round;
  }
  .leg .out {
    stroke: #2a2a30;
    stroke-width: 6.2;
  }
  .leg .in {
    stroke: #f7f7f9;
    stroke-width: 3.4;
  }
  .leg.far .in {
    stroke: #d9d9df;
  }
  .tail .out {
    stroke: #2a2a30;
    stroke-width: 4.6;
  }
  .tail .in {
    stroke: #f7f7f9;
    stroke-width: 1.8;
  }
  .ink {
    fill: #1d1d22;
  }
  .mouth {
    fill: #3a2c2e;
  }
  .line {
    fill: none;
    stroke: #1d1d22;
    stroke-width: 1.15;
    stroke-linecap: round;
  }
  .shadow {
    fill: rgba(0, 0, 0, 0.2);
    transform-origin: 34px 50.6px;
  }
  .whole,
  .settle {
    transform-origin: 34px 50px;
  }

  /* ---- Changing pose: it settles into each one instead of snapping. ---- */
  .sit .settle {
    animation: sit-down 380ms var(--ease-out);
  }
  @keyframes sit-down {
    from {
      transform: translateY(-3px) scaleY(1.06);
    }
    55% {
      transform: scaleY(0.95);
    }
  }
  .lie .settle {
    animation: lie-down 520ms var(--ease-out);
  }
  @keyframes lie-down {
    from {
      transform: translateY(-6px) scaleY(1.12);
    }
    60% {
      transform: scaleY(0.96);
    }
  }
  .stand .settle {
    animation: stand-up 300ms var(--ease-out);
  }
  @keyframes stand-up {
    from {
      transform: scaleY(0.94);
    }
  }

  /* ---- Moving ---- */
  /* Walk: a trot, diagonal legs together; the body bobs twice a stride, the ear lags behind. */
  .walk .leg.a {
    animation: swing 0.46s ease-in-out infinite;
  }
  .walk .leg.b {
    animation: swing 0.46s ease-in-out infinite reverse;
  }
  .walk .whole {
    animation: bob 0.23s ease-in-out infinite alternate;
  }
  .walk .ear {
    animation: flop 0.23s ease-in-out 0.06s infinite alternate;
  }
  .walk .tail {
    animation: sway 0.46s ease-in-out infinite alternate;
  }
  @keyframes swing {
    0%,
    100% {
      transform: rotate(18deg);
    }
    50% {
      transform: rotate(-18deg);
    }
  }
  @keyframes bob {
    to {
      transform: translateY(-1.2px);
    }
  }
  @keyframes flop {
    to {
      transform: rotate(-9deg);
    }
  }
  @keyframes sway {
    from {
      transform: rotate(-8deg);
    }
    to {
      transform: rotate(10deg);
    }
  }

  /* Run: a gallop, front and back pairs together, the body rocking over them. */
  .run .leg.front {
    animation: gallop-front 0.3s ease-in-out infinite;
  }
  .run .leg.back {
    animation: gallop-back 0.3s ease-in-out infinite;
  }
  .run .whole {
    animation: rock 0.3s ease-in-out infinite;
  }
  .run .ear {
    animation: flap 0.3s ease-in-out 0.05s infinite;
  }
  .run .tail {
    transform: rotate(-38deg);
  }
  @keyframes gallop-front {
    0%,
    100% {
      transform: rotate(-38deg);
    }
    50% {
      transform: rotate(26deg);
    }
  }
  /* Opposite to the front: stretched out at 0%, gathered under at 50%. */
  @keyframes gallop-back {
    0%,
    100% {
      transform: rotate(34deg);
    }
    50% {
      transform: rotate(-30deg);
    }
  }
  @keyframes rock {
    0%,
    100% {
      transform: translateY(0) rotate(-4deg);
    }
    50% {
      transform: translateY(-3.5px) rotate(4deg);
    }
  }
  @keyframes flap {
    0%,
    100% {
      transform: rotate(-30deg);
    }
    50% {
      transform: rotate(-12deg);
    }
  }

  /* ---- Up and down the screen ---- */
  /* It leans by the angle it's travelling at, gliding between angles as the path bends; the
     head turns back about halfway so it keeps looking where it's going. Steep up: ear streaming
     back, a longer reaching stride. Steep down: shorter careful steps. */
  .tilt {
    transform-origin: 34px 48px;
    transition: transform 220ms ease-out;
  }
  .walk .tilt,
  .run .tilt {
    transform: rotate(var(--slope));
  }
  .walk .head,
  .run .head {
    transform: rotate(calc(var(--slope) * -0.45));
  }
  .walk.up .ear {
    animation-name: flop-back;
  }
  @keyframes flop-back {
    from {
      transform: rotate(-14deg);
    }
    to {
      transform: rotate(-24deg);
    }
  }
  .walk.up .leg.a,
  .walk.up .leg.b {
    animation-name: reach;
  }
  @keyframes reach {
    0%,
    100% {
      transform: rotate(24deg);
    }
    50% {
      transform: rotate(-24deg);
    }
  }
  .walk.down .leg.a,
  .walk.down .leg.b {
    animation-name: step;
  }
  @keyframes step {
    0%,
    100% {
      transform: rotate(11deg);
    }
    50% {
      transform: rotate(-11deg);
    }
  }
  .walk.down .whole {
    animation-name: bob-down;
  }
  @keyframes bob-down {
    to {
      transform: translateY(-0.6px);
    }
  }

  /* Alert: ears up, a little hop of attention before it sets off. */
  .alert .ear {
    transform: rotate(-36deg);
    transition: transform 120ms;
  }
  .alert .whole {
    animation: perk 280ms var(--ease-out);
  }
  @keyframes perk {
    40% {
      transform: translateY(-3px);
    }
  }
  /* Skid: it leans back as it stops, then settles. */
  .skid .whole {
    animation: skid 380ms var(--ease-out);
  }
  @keyframes skid {
    30% {
      transform: rotate(-7deg) translateX(-1px);
    }
    70% {
      transform: rotate(2deg);
    }
  }

  /* ---- Leaving and coming home ---- */
  /* Wake: drops out of the island, lands with a squash, shakes itself off. */
  .wake .whole {
    animation: drop-in 900ms var(--ease-out);
  }
  .wake .shadow {
    animation: shadow-in 900ms var(--ease-out);
  }
  @keyframes drop-in {
    0% {
      transform: translateY(-34px) scale(0.6);
      opacity: 0;
    }
    12% {
      opacity: 1;
    }
    32% {
      transform: translateY(0) scale(1.08, 0.86);
      animation-timing-function: ease-out;
    }
    44% {
      transform: none;
    }
    56% {
      transform: rotate(-5deg);
    }
    68% {
      transform: rotate(5deg);
    }
    80% {
      transform: rotate(-2deg);
    }
  }
  @keyframes shadow-in {
    from {
      transform: scale(0.3);
      opacity: 0;
    }
    32% {
      transform: none;
      opacity: 1;
    }
  }
  /* Climb: crouches, springs up into the island and is gone. */
  .climb .whole {
    animation: spring-up 650ms ease-in forwards;
  }
  .climb .shadow {
    animation: shadow-out 650ms ease-in forwards;
  }
  @keyframes spring-up {
    35% {
      transform: scale(1.08, 0.84);
      animation-timing-function: cubic-bezier(0.3, 0, 0.2, 1);
    }
    100% {
      transform: translateY(-40px) scale(0.55);
      opacity: 0;
    }
  }
  @keyframes shadow-out {
    35% {
      transform: scale(1.1);
    }
    to {
      transform: scale(0.2);
      opacity: 0;
    }
  }

  /* ---- Tricks ---- */
  .happy .tail,
  .wag .tail,
  .nudge .tail {
    animation: wag 0.2s ease-in-out infinite alternate;
  }
  .wag .tail {
    animation-iteration-count: 12;
  }
  .nudge .tail {
    animation-iteration-count: 10;
  }
  .happy .whole {
    animation: wiggle 0.4s ease-in-out infinite alternate;
  }
  .happy .head {
    transform: rotate(-6deg);
  }
  .happy .ear {
    transform: rotate(-14deg);
  }
  @keyframes wag {
    from {
      transform: rotate(-18deg);
    }
    to {
      transform: rotate(20deg);
    }
  }
  @keyframes wiggle {
    to {
      transform: rotate(1.6deg);
    }
  }

  .head {
    transition: transform 320ms var(--ease-out);
  }
  .ear {
    transition: transform 260ms var(--ease-out);
  }

  /* Yawn: head back, mouth open, a little shiver at the end. */
  .yawn .head {
    animation: yawn 1.6s ease-in-out;
  }
  @keyframes yawn {
    30%,
    68% {
      transform: rotate(-16deg);
    }
    78% {
      transform: rotate(-2deg);
    }
    86% {
      transform: rotate(-5deg);
    }
  }

  /* Scratch: the hind paw goes at the ear, head tipped into it. */
  .scratch .head {
    transform: rotate(10deg);
  }
  .scratch .scratcher {
    animation: scratch 0.12s linear 12 alternate;
  }
  @keyframes scratch {
    to {
      transform: rotate(9deg);
    }
  }

  /* Sniff: nose to the ground, a few quick sniffs. */
  .sniff .head {
    animation: sniff 1.8s ease-in-out;
  }
  @keyframes sniff {
    18%,
    82% {
      transform: rotate(30deg) translateY(4px);
    }
    30%,
    50%,
    70% {
      transform: rotate(28deg) translate(0.8px, 4px);
    }
    40%,
    60% {
      transform: rotate(31deg) translate(-0.4px, 4px);
    }
  }
  .lie.sniff .head {
    animation-name: sniff-low;
  }
  @keyframes sniff-low {
    30%,
    70% {
      transform: rotate(6deg) translateX(1px);
    }
    50% {
      transform: rotate(4deg);
    }
  }

  /* Look up at the cursor; listening also cocks the head and lifts the ear. */
  .look .head {
    transform: rotate(-14deg);
  }
  .listen .head {
    transform: rotate(-9deg);
  }
  .listen .ear,
  .look .ear {
    transform: rotate(-30deg);
  }

  .bob .head {
    animation: nod 0.9s ease-in-out 2;
  }
  @keyframes nod {
    50% {
      transform: rotate(10deg) translateY(1px);
    }
  }
  .bob .whole {
    animation: groove 0.9s ease-in-out 2;
  }
  @keyframes groove {
    50% {
      transform: scaleY(0.97);
    }
  }

  /* Hop: crouch, up with a stretch, land with a squash. */
  .hop .whole {
    animation: hop 560ms;
  }
  .hop .shadow {
    animation: shadow-hop 560ms;
  }
  @keyframes hop {
    0% {
      animation-timing-function: ease-out;
    }
    18% {
      transform: scale(1.06, 0.88);
      animation-timing-function: cubic-bezier(0.2, 0.7, 0.4, 1);
    }
    48% {
      transform: translateY(-13px) scale(0.95, 1.06);
      animation-timing-function: cubic-bezier(0.6, 0, 0.8, 0.4);
    }
    78% {
      transform: scale(1.07, 0.9);
      animation-timing-function: ease-out;
    }
  }
  @keyframes shadow-hop {
    48% {
      transform: scale(0.6);
      opacity: 0.5;
    }
  }

  /* Nudge: bumps its head into the cursor, wagging. */
  .nudge .head {
    animation: bump 0.45s ease-in-out 3;
  }
  @keyframes bump {
    50% {
      transform: translateX(3px) rotate(7deg);
    }
  }

  /* Bark: head up, a little recoil. */
  .bark .head {
    animation: woof 0.32s ease-out 2;
  }
  .bark .whole {
    animation: recoil 0.32s ease-out 2;
  }
  @keyframes woof {
    35% {
      transform: rotate(-14deg);
    }
  }
  @keyframes recoil {
    35% {
      transform: translateX(-1px) scaleY(1.03);
    }
  }
</style>
