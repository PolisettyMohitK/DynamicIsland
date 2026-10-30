<script lang="ts">
  // Choco's reply: the words drift in one by one; "- " lines become bullets; a live card may follow.
  // Long replies scroll inside, with a soft fade at the edge.
  import type { Reply } from '$lib/commands';
  import AnswerPanel from './AnswerPanel.svelte';
  import ChocoFace from './ChocoFace.svelte';
  import Wave from './Wave.svelte';

  let {
    reply,
    heard,
    followUp,
    level,
    width,
    onrevealed,
  }: { reply: Reply; heard: string; followUp: boolean; level: number; width: number; onrevealed: () => void } = $props();

  /** Tallest the text gets before it scrolls. */
  const MAX_TEXT = 380;
  // The whole reply takes at most this long to appear, however long it is.
  const REVEAL_MS = 1600;
  const WORD_MS = 34;

  type Block = { bullet: boolean; words: { w: string; i: number }[] };
  const blocks = $derived.by(() => {
    let i = 0;
    return reply.text
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean)
      .map((l): Block => {
        const bullet = /^[-•*]\s+/.test(l);
        return { bullet, words: l.replace(/^[-•*]\s+/, '').split(/\s+/).map((w) => ({ w, i: i++ })) };
      });
  });
  const count = $derived(blocks.reduce((n, b) => n + b.words.length, 0));
  const step = $derived(Math.min(WORD_MS, REVEAL_MS / Math.max(1, count)));

  $effect(() => {
    const done = setTimeout(onrevealed, count * step + 300);
    return () => clearTimeout(done);
  });

  let scroller = $state<HTMLDivElement>();
  let atTop = $state(true);
  let atEnd = $state(true);
  function edges() {
    if (!scroller) return;
    atTop = scroller.scrollTop < 2;
    atEnd = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2;
  }
  $effect(() => {
    void blocks;
    requestAnimationFrame(edges);
  });
</script>

<div class="answer" class:bad={reply.tone === 'bad'} style="width: {width}px">
  <div class="top">
    <ChocoFace mood={reply.tone === 'bad' ? 'sad' : followUp ? 'alert' : 'happy'} size={17} {level} />
    {#if followUp}
      <span class="listening"><Wave {level} bars={10} /></span>
    {:else if heard}
      <span class="heard">{heard}</span>
    {/if}
  </div>

  {#if reply.text}
    <div
      class="text"
      class:fade-top={!atTop}
      class:fade-end={!atEnd}
      style="max-height: {MAX_TEXT}px; --step: {step}ms"
      bind:this={scroller}
      onscroll={edges}
      onwheel={(e) => e.stopPropagation()}
    >
      {#each blocks as b}
        <p class:bullet={b.bullet}>
          {#each b.words as { w, i }}<span style="--i: {i}">{w}</span>{' '}{/each}
        </p>
      {/each}
    </div>
  {/if}

  {#if reply.panel}<AnswerPanel panel={reply.panel} />{/if}
</div>

<style>
  .answer {
    padding: 12px 22px 16px;
    display: flex;
    flex-direction: column;
  }
  .top {
    height: 20px;
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text);
  }
  .heard {
    min-width: 0;
    font-size: 12px;
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    animation: in 300ms var(--ease-out) both;
  }
  .listening {
    animation: in 300ms var(--ease-out) both;
  }
  .text {
    margin-top: 8px;
    overflow-y: auto;
    scrollbar-width: none;
    font-size: 14px;
    line-height: 21px;
    font-weight: 500;
    letter-spacing: 0.1px;
    --fade: 22px;
  }
  .text::-webkit-scrollbar {
    display: none;
  }
  .fade-end {
    mask-image: linear-gradient(to bottom, #000 calc(100% - var(--fade)), transparent);
  }
  .fade-top {
    mask-image: linear-gradient(to bottom, transparent, #000 var(--fade));
  }
  .fade-top.fade-end {
    mask-image: linear-gradient(to bottom, transparent, #000 var(--fade), #000 calc(100% - var(--fade)), transparent);
  }
  p {
    margin: 0;
  }
  p + p {
    margin-top: 6px;
  }
  p.bullet {
    position: relative;
    padding-left: 16px;
  }
  p.bullet + p.bullet {
    margin-top: 3px;
  }
  p.bullet::before {
    content: '';
    position: absolute;
    left: 3px;
    top: 9px;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--text-dim);
  }
  .bad .text {
    color: var(--text-dim);
  }
  /* Each word drifts up out of a soft blur. */
  .text span {
    display: inline-block;
    animation: word 460ms var(--ease-out) both;
    animation-delay: calc(var(--i) * var(--step));
  }
  @keyframes word {
    from {
      opacity: 0;
      filter: blur(4px);
      transform: translateY(3px);
    }
  }
  @keyframes in {
    from {
      opacity: 0;
    }
  }
</style>
