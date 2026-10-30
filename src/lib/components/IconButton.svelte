<script lang="ts">
  import type { IconName } from '$lib/icons';
  import Icon from './Icon.svelte';

  let {
    icon,
    label,
    onclick,
    size = 32,
    primary = false,
    active = false,
    quiet = false,
    filled = false,
    accent = 'var(--text)',
  }: {
    icon: IconName;
    label: string;
    onclick: () => void;
    size?: number;
    /** Filled with the accent colour, with a soft glow. */
    primary?: boolean;
    /** Tinted with the accent colour (a toggle that is on). */
    active?: boolean;
    /** No background until hovered. */
    quiet?: boolean;
    filled?: boolean;
    accent?: string;
  } = $props();
</script>

<button
  class:primary
  class:active
  class:quiet
  style="--size: {size}px; --accent: {accent}"
  aria-label={label}
  title={label}
  {onclick}
>
  {#key icon}
    <span class="glyph"><Icon name={icon} size={Math.round(size * 0.52)} {filled} /></span>
  {/key}
</button>

<style>
  button {
    width: var(--size);
    height: var(--size);
    flex: none;
    padding: 0;
    border: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--pill);
    box-shadow: inset 0 0.5px 0 rgba(255, 255, 255, 0.12);
    color: var(--text);
    cursor: pointer;
    transition:
      background 160ms,
      color 160ms,
      box-shadow 240ms,
      transform 260ms var(--spring);
  }
  button:hover {
    background: var(--pill-hover);
    transform: scale(1.06);
  }
  button:active {
    transform: scale(0.96);
    transition-duration: 80ms;
  }
  .quiet {
    background: transparent;
    box-shadow: none;
    color: var(--text-dim);
  }
  .quiet:hover {
    color: var(--text);
  }
  .active {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .active:hover {
    background: color-mix(in srgb, var(--accent) 24%, transparent);
  }
  .primary {
    color: #fff;
    background: var(--accent);
    box-shadow:
      inset 0 0.5px 0 rgba(255, 255, 255, 0.35),
      0 4px 16px color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .primary:hover {
    background: color-mix(in srgb, var(--accent) 88%, #fff);
    box-shadow:
      inset 0 0.5px 0 rgba(255, 255, 255, 0.35),
      0 6px 22px color-mix(in srgb, var(--accent) 60%, transparent);
  }
  /* A swapped glyph (play ↔ pause) pops in instead of jumping. */
  .glyph {
    display: grid;
    animation: pop 260ms var(--spring);
  }
  @keyframes pop {
    from {
      transform: scale(0.9);
      opacity: 0;
    }
  }
</style>
