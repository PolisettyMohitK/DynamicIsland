<script lang="ts" generics="T extends string">
  // Two or more choices with a highlight that slides to the picked one.
  let {
    options,
    value,
    onchange,
    accent = 'var(--text)',
  }: { options: { value: T; label: string }[]; value: T; onchange: (v: T) => void; accent?: string } = $props();

  const index = $derived(options.findIndex((o) => o.value === value));
</script>

<div class="seg" style="--n: {options.length}; --i: {index}; --accent: {accent}" role="radiogroup">
  <span class="thumb"></span>
  {#each options as o}
    <button role="radio" aria-checked={o.value === value} class:on={o.value === value} onclick={() => onchange(o.value)}>
      {o.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    position: relative;
    height: 28px;
    padding: 2px;
    border-radius: 14px;
    display: grid;
    grid-template-columns: repeat(var(--n), 1fr);
    background: var(--pill);
    box-shadow: inset 0 0.5px 0 rgba(255, 255, 255, 0.1);
  }
  .thumb {
    position: absolute;
    top: 2px;
    bottom: 2px;
    left: 2px;
    width: calc((100% - 4px) / var(--n));
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 22%, rgba(255, 255, 255, 0.08));
    box-shadow: inset 0 0.5px 0 rgba(255, 255, 255, 0.18);
    transform: translateX(calc(100% * var(--i)));
    transition: transform 360ms var(--spring);
  }
  button {
    position: relative;
    padding: 0 12px;
    border: none;
    background: none;
    color: var(--text-dim);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    transition: color 200ms;
  }
  button:hover,
  button.on {
    color: var(--text);
  }
</style>
