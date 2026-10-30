<script lang="ts">
  // − value + for a number setting.
  let { value, onstep, label }: { value: string; onstep: (steps: number) => void; label: string } = $props();
</script>

<span class="stepper">
  <button aria-label="Less {label}" onclick={() => onstep(-1)}>−</button>
  <b>{value}</b>
  <button aria-label="More {label}" onclick={() => onstep(1)}>+</button>
</span>

<style>
  .stepper {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  b {
    min-width: 58px;
    text-align: center;
    font-size: 12.5px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  button {
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 50%;
    padding: 0;
    background: var(--pill);
    color: var(--text);
    font: inherit;
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    transition:
      background 160ms,
      transform 240ms var(--spring);
  }
  button:hover {
    background: var(--pill-hover);
  }
  button:active {
    transform: scale(0.85);
  }
</style>
