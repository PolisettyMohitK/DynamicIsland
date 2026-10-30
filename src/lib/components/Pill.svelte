<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    onclick,
    primary = false,
    active = false,
    accent = 'var(--text)',
    label,
    children,
  }: {
    onclick: () => void;
    /** Filled with the page's accent colour. */
    primary?: boolean;
    active?: boolean;
    accent?: string;
    label?: string;
    children: Snippet;
  } = $props();
</script>

<button class:primary class:active style="--accent: {accent}" aria-label={label} {onclick}>
  {@render children()}
</button>

<style>
  button {
    height: 28px;
    padding: 0 12px;
    border: none;
    border-radius: 14px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    background: var(--pill);
    box-shadow: inset 0 0.5px 0 rgba(255, 255, 255, 0.12);
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background 160ms,
      color 160ms,
      transform 260ms var(--spring);
  }
  button:hover {
    background: var(--pill-hover);
    transform: scale(1.04);
  }
  button:active {
    transform: scale(0.96);
    transition-duration: 80ms;
  }
  button.active {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  button.primary {
    background: var(--accent);
    color: #fff;
    box-shadow:
      inset 0 0.5px 0 rgba(255, 255, 255, 0.35),
      0 4px 14px color-mix(in srgb, var(--accent) 40%, transparent);
  }
  button.primary:hover {
    background: color-mix(in srgb, var(--accent) 88%, #fff);
  }
</style>
