<script lang="ts">
  import { nowPlaying } from '$lib/media.svelte';

  const pad = (n: number) => String(Math.floor(n)).padStart(2, '0');
  const fmt = (ms: number) => `${pad(ms / 60000)}:${pad((ms / 1000) % 60)}`;

  const duration = $derived(nowPlaying.media?.durationMs ?? 0);
  const position = $derived(nowPlaying.position);

  function seek(e: MouseEvent) {
    const bar = e.currentTarget as HTMLElement;
    const { left, width } = bar.getBoundingClientRect();
    nowPlaying.seek(Math.max(0, Math.min(1, (e.clientX - left) / width)) * duration);
  }
</script>

{#if duration > 0}
  <div class="progress">
    <span>{fmt(position)}</span>
    <button class="bar" aria-label="Seek" onclick={seek}>
      <i style="width: {(position / duration) * 100}%"></i>
    </button>
    <span>-{fmt(duration - position)}</span>
  </div>
{/if}

<style>
  .progress {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 11px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .bar {
    flex: 1;
    height: 16px;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
    position: relative;
  }
  /* The visible track is thin; the button is taller so it's easy to hit. */
  .bar::before,
  .bar i {
    content: '';
    position: absolute;
    left: 0;
    top: 6.5px;
    height: 3px;
    border-radius: 2px;
    transition:
      height 200ms var(--ease-out),
      top 200ms var(--ease-out);
  }
  .bar::before {
    right: 0;
    background: var(--track);
  }
  .bar i {
    background: var(--text);
    transition:
      width 200ms linear,
      height 200ms var(--ease-out),
      top 200ms var(--ease-out);
  }
  /* The track thickens under the cursor so it reads as grabbable. */
  .bar:hover::before,
  .bar:hover i {
    top: 5px;
    height: 6px;
    border-radius: 3px;
  }
</style>
