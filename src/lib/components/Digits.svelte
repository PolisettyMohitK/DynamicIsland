<script lang="ts">
  // Text whose characters roll into place when they change, like a flip counter.
  let { value }: { value: string } = $props();
</script>

<span class="digits" aria-label={value}>
  {#each value.split('') as ch, i (i)}
    {#key ch}
      <span class="ch" aria-hidden="true">{ch}</span>
    {/key}
  {/each}
</span>

<style>
  .digits {
    display: inline-flex;
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    /* Room for the roll without clipping descenders. */
    padding: 0.05em 0;
    margin: -0.05em 0;
  }
  .ch {
    display: inline-block;
    white-space: pre;
    animation: roll 380ms var(--ease-out);
  }
  @keyframes roll {
    from {
      transform: translateY(55%);
      opacity: 0;
      filter: blur(2px);
    }
  }
</style>
