<script lang="ts">
  import { status } from '$lib/status.svelte';

  const R = 5;
  const C = 2 * Math.PI * R;

  const battery = $derived(status.current?.battery ?? null);
  const colour = $derived(
    !battery || battery.charging || battery.percent > 20 ? 'var(--ok)' : battery.percent > 10 ? '#f0bf7f' : 'var(--bad)',
  );
</script>

<!-- Battery level as a ring; a full green ring on machines without a battery. -->
<svg width="14" height="14" viewBox="0 0 14 14" aria-label={battery ? `Battery ${battery.percent}%` : undefined}>
  <circle cx="7" cy="7" r={R} fill="none" stroke="rgba(255,255,255,0.18)" stroke-width="2" />
  <circle
    cx="7"
    cy="7"
    r={R}
    fill="none"
    stroke={colour}
    stroke-width="2"
    stroke-linecap="round"
    stroke-dasharray="{(C * (battery?.percent ?? 100)) / 100} {C}"
    transform="rotate(-90 7 7)"
  />
  {#if battery?.charging}
    <path d="M7.6 3.8L5.2 7.4h2l-.6 2.8 2.4-3.6h-2z" fill={colour} />
  {/if}
</svg>

<style>
  svg {
    flex: none;
  }
</style>
