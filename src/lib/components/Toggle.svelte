<script lang="ts">
  // An on/off switch.
  let { on, onchange, label }: { on: boolean; onchange: (on: boolean) => void; label: string } = $props();
</script>

<button role="switch" aria-checked={on} aria-label={label} class:on onclick={() => onchange(!on)}><i></i></button>

<style>
  button {
    width: 34px;
    height: 20px;
    flex: none;
    padding: 2px;
    border: none;
    border-radius: 10px;
    background: var(--track);
    cursor: pointer;
    transition: background 220ms;
  }
  button.on {
    background: color-mix(in srgb, var(--text) 70%, transparent);
  }
  i {
    display: block;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
    transition: transform 320ms var(--spring);
  }
  .on i {
    transform: translateX(14px);
    background: #1a1a1e;
  }
</style>
