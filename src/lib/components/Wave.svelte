<script lang="ts">
  // Your voice as a row of soft bars that scroll by; while thinking, a slow ripple instead.
  import { untrack } from 'svelte';
  let { level, thinking = false, bars = 26 }: { level: number; thinking?: boolean; bars?: number } = $props();

  let history = $state<number[]>([]);
  $effect(() => {
    const l = level;
    if (thinking) return;
    // Square root lifts quiet speech so it still shows.
    untrack(() => (history = [...history.slice(-(bars - 1)), Math.sqrt(l)]));
  });
  const heights = $derived(Array.from({ length: bars }, (_, i) => history[i - (bars - history.length)] ?? 0));
</script>

<div class="wave" class:thinking style="--n: {bars}">
  {#each heights as h, i}
    <i style="--h: {h}; --i: {i}"></i>
  {/each}
</div>

<style>
  .wave {
    display: flex;
    align-items: center;
    gap: 3px;
    height: 22px;
  }
  i {
    width: 2.5px;
    height: calc(3px + var(--h) * 19px);
    border-radius: 2px;
    background: var(--text);
    opacity: calc(0.35 + var(--h) * 0.65);
    transition:
      height 120ms var(--ease-out),
      opacity 120ms;
  }
  .thinking i {
    height: 3px;
    opacity: 0.3;
    animation: ripple 1.4s ease-in-out infinite;
    animation-delay: calc(var(--i) * 45ms);
  }
  @keyframes ripple {
    40% {
      height: 9px;
      opacity: 0.8;
    }
  }
</style>
