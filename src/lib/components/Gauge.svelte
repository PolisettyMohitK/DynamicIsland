<script lang="ts">
  import type { Snippet } from 'svelte';

  // A progress ring with something in the middle.
  let {
    value,
    size = 76,
    stroke = 6,
    accent,
    glow = false,
    children,
  }: { value: number; size?: number; stroke?: number; accent: string; glow?: boolean; children?: Snippet } = $props();

  const r = $derived((size - stroke) / 2);
  const c = $derived(2 * Math.PI * r);
</script>

<div class="gauge" class:glow style="width: {size}px; height: {size}px; color: {accent}">
  <svg width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true">
    <circle cx={size / 2} cy={size / 2} {r} fill="none" stroke="var(--track)" stroke-width={stroke} />
    <circle
      class="arc"
      cx={size / 2}
      cy={size / 2}
      {r}
      fill="none"
      stroke="currentColor"
      stroke-width={stroke}
      stroke-linecap="round"
      opacity={value > 0 ? 1 : 0}
      stroke-dasharray="{c * Math.min(1, Math.max(0, value))} {c}"
      transform="rotate(-90 {size / 2} {size / 2})"
    />
  </svg>
  <div class="inner">{@render children?.()}</div>
</div>

<style>
  .gauge {
    position: relative;
    flex: none;
  }
  svg {
    overflow: visible;
  }
  .arc {
    transition:
      stroke-dasharray 600ms var(--ease-out),
      opacity 300ms;
  }
  .glow .arc {
    filter: drop-shadow(0 0 5px currentColor);
  }
  .inner {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }
</style>
