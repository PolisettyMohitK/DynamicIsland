<script lang="ts">
  // Circle to ask: a dimmed screen to draw on. The circled part stays bright; ask about it
  // by voice (it starts listening as soon as you let go) or by typing, and the answer
  // appears beside it. Esc closes; drawing again starts over.
  import '$lib/theme.css';
  import { tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from '$lib/components/Icon.svelte';
  import Wave from '$lib/components/Wave.svelte';

  type Box = { x: number; y: number; w: number; h: number };
  const CARD_W = 380;
  const GAP = 14;
  /** Smaller than this is a click, not a circle. */
  const MIN_SIDE = 16;

  let points = $state<[number, number][]>([]);
  let drawing = $state(false);
  let box = $state<Box | null>(null);
  let stage = $state<'draw' | 'ask' | 'thinking' | 'answer'>('draw');
  let question = $state('');
  let listening = $state(false);
  let level = $state(0);
  let answer = $state('');
  let failed = $state(false);
  let field: HTMLInputElement | undefined = $state();
  let run = 0;

  const path = $derived(points.length ? 'M' + points.map(([x, y]) => `${x} ${y}`).join('L') + (drawing ? '' : 'Z') : '');
  const card = $derived.by(() => {
    if (!box) return { left: 0, top: 0 };
    const left = Math.min(Math.max(GAP, box.x + box.w / 2 - CARD_W / 2), innerWidth - CARD_W - GAP);
    const below = box.y + box.h + GAP;
    // Below the circle if there's room, else above it, else inside the bottom of it.
    const top = below + 180 < innerHeight ? below : Math.max(GAP, box.y - GAP - 180);
    return { left, top };
  });

  $effect(() => {
    const off = [
      listen('circle-open', reset),
      listen<number>('circle-level', (e) => (level = e.payload)),
    ];
    return () => off.forEach((p) => p.then((f) => f()));
  });

  function reset() {
    run++;
    invoke('circle_stop_listening');
    points = [];
    drawing = false;
    box = null;
    stage = 'draw';
    question = '';
    listening = false;
    answer = '';
    failed = false;
  }

  function close() {
    reset();
    invoke('circle_close');
  }

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    reset();
    drawing = true;
    points = [[e.clientX, e.clientY]];
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  }

  function move(e: PointerEvent) {
    if (!drawing) return;
    const [lx, ly] = points[points.length - 1];
    // Skip points closer than a few px: smoother line, smaller path.
    if (Math.hypot(e.clientX - lx, e.clientY - ly) > 3) points.push([e.clientX, e.clientY]);
  }

  async function up() {
    if (!drawing) return;
    drawing = false;
    const xs = points.map((p) => p[0]);
    const ys = points.map((p) => p[1]);
    const b = { x: Math.min(...xs), y: Math.min(...ys), w: Math.max(...xs) - Math.min(...xs), h: Math.max(...ys) - Math.min(...ys) };
    if (b.w < MIN_SIDE || b.h < MIN_SIDE) {
      points = [];
      return;
    }
    box = b;
    stage = 'ask';
    await tick();
    field?.focus();
    hear();
  }

  async function hear() {
    const mine = run;
    listening = true;
    const said = await invoke<string>('circle_listen').catch(() => '');
    if (mine !== run) return;
    listening = false;
    level = 0;
    // Typing took over, or nothing was said: leave it to the text field.
    if (said && stage === 'ask' && !question.trim()) {
      question = said;
      ask();
    }
  }

  function typing() {
    if (listening) {
      listening = false;
      invoke('circle_stop_listening');
    }
  }

  async function ask() {
    if (!box || stage === 'thinking') return;
    const mine = run;
    stage = 'thinking';
    invoke('circle_stop_listening');
    // The circled box, padded a little, in physical pixels.
    const pad = 6;
    const dpr = devicePixelRatio;
    const x = Math.max(0, box.x - pad);
    const y = Math.max(0, box.y - pad);
    try {
      const text = await invoke<string>('circle_ask', {
        x: Math.round(x * dpr),
        y: Math.round(y * dpr),
        w: Math.round((box.w + pad * 2) * dpr),
        h: Math.round((box.h + pad * 2) * dpr),
        question,
      });
      if (mine !== run) return;
      answer = text || 'No answer came back.';
      failed = false;
    } catch (e) {
      if (mine !== run) return;
      answer = String(e);
      failed = true;
    }
    stage = 'answer';
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="stage" onpointerdown={down} onpointermove={move} onpointerup={up}>
  <svg width="100%" height="100%">
    <defs>
      <mask id="hole">
        <rect width="100%" height="100%" fill="white" />
        {#if path && !drawing}<path d={path} fill="black" />{/if}
      </mask>
    </defs>
    <rect class="veil" width="100%" height="100%" mask="url(#hole)" />
    {#if path}<path class="line" d={path} />{/if}
  </svg>

  {#if stage === 'draw' && !drawing}
    <div class="hint">Draw around anything to ask about it <kbd>Esc</kbd></div>
  {/if}

  {#if box && stage !== 'draw'}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="card" style="left: {card.left}px; top: {card.top}px; width: {CARD_W}px" onpointerdown={(e) => e.stopPropagation()}>
      {#if stage === 'answer'}
        <p class="question">{question || 'What is this?'}</p>
        <div class="answer" class:failed>
          {#each answer.split('\n').filter((l) => l.trim()) as line}
            {#if line.trimStart().startsWith('- ')}<p class="bullet">{line.trimStart().slice(2)}</p>{:else}<p>{line}</p>{/if}
          {/each}
        </div>
        <button class="again" onclick={() => ((stage = 'ask'), (question = ''), tick().then(() => field?.focus()))}>Ask something else</button>
      {:else}
        <form onsubmit={(e) => (e.preventDefault(), ask())}>
          {#if listening}
            <Wave {level} bars={10} />
          {:else}
            <Icon name="sparkle" size={16} />
          {/if}
          <input
            bind:this={field}
            bind:value={question}
            oninput={typing}
            placeholder={listening ? 'Listening… or type' : 'Ask about this'}
            disabled={stage === 'thinking'}
            spellcheck="false"
          />
          {#if stage === 'thinking'}<Wave level={0} thinking bars={10} />{/if}
        </form>
      {/if}
    </div>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
  }
  .stage {
    position: fixed;
    inset: 0;
    cursor: crosshair;
    font-family: var(--font);
    color: var(--text);
  }
  svg {
    position: absolute;
    inset: 0;
  }
  .veil {
    fill: rgba(0, 0, 0, 0.42);
    animation: dim 220ms var(--ease-out);
  }
  @keyframes dim {
    from {
      opacity: 0;
    }
  }
  .line {
    fill: none;
    stroke: white;
    stroke-width: 3;
    stroke-linecap: round;
    stroke-linejoin: round;
    filter: drop-shadow(0 0 6px rgba(255, 255, 255, 0.8));
  }
  .hint {
    position: absolute;
    top: 64px;
    left: 50%;
    translate: -50% 0;
    padding: 10px 16px;
    border-radius: 20px;
    background: rgba(20, 20, 22, 0.85);
    backdrop-filter: blur(12px);
    font-size: 13px;
    pointer-events: none;
    display: flex;
    align-items: center;
    gap: 10px;
    animation: pop 260ms var(--ease-out);
  }
  kbd {
    padding: 1px 6px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.12);
    font: 600 11px var(--font);
    color: var(--text-dim);
  }
  .card {
    position: absolute;
    padding: 12px 14px;
    border-radius: 20px;
    background: rgba(20, 20, 22, 0.9);
    backdrop-filter: blur(16px) saturate(1.4);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.12),
      0 18px 50px rgba(0, 0, 0, 0.5);
    cursor: default;
    animation: pop 260ms var(--ease-out);
  }
  @keyframes pop {
    from {
      opacity: 0;
      scale: 0.96;
      filter: blur(4px);
    }
  }
  form {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-dim);
  }
  input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: none;
    color: var(--text);
    font: 14px var(--font);
  }
  input::placeholder {
    color: var(--text-faint);
  }
  .question {
    margin: 0 0 6px;
    font-size: 12px;
    color: var(--text-dim);
  }
  .answer p {
    margin: 0 0 6px;
    font-size: 13.5px;
    line-height: 1.45;
  }
  .answer .bullet {
    padding-left: 14px;
    position: relative;
  }
  .answer .bullet::before {
    content: '';
    position: absolute;
    left: 3px;
    top: 0.6em;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--text-dim);
  }
  .answer.failed p {
    color: var(--bad);
  }
  .again {
    margin-top: 4px;
    padding: 0;
    border: none;
    background: none;
    color: var(--text-dim);
    font: 600 12px var(--font);
    cursor: pointer;
  }
  .again:hover {
    color: var(--text);
  }
</style>
