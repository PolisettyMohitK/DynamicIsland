<script lang="ts">
  import { clock } from '$lib/clock.svelte';
  import { focus, LENGTH } from '$lib/focus.svelte';
  import { hhmm, mmss } from '$lib/store';
  import Digits from '../Digits.svelte';
  import Gauge from '../Gauge.svelte';
  import IconButton from '../IconButton.svelte';
  import Segmented from '../Segmented.svelte';

  const ACCENT = 'var(--accent-focus)';
  const KINDS = [
    { value: 'focus', label: 'Focus' },
    { value: 'break', label: 'Break' },
  ] as const;

  const left = $derived(focus.remaining(clock.now));
  const caption = $derived(
    focus.running ? `Ends at ${hhmm(clock.now + left)}` : `${LENGTH[focus.kind] / 60_000} minute ${focus.kind === 'focus' ? 'session' : 'break'}`,
  );
</script>

<div class="text">
  <b class="time"><Digits value={mmss(Math.ceil(left / 1000) * 1000)} /></b>
  <span class="caption">{caption}</span>
  <div class="row">
    <Segmented options={[...KINDS]} value={focus.kind} accent={ACCENT} onchange={(k) => focus.choose(k)} />
    <IconButton icon="reset" label="Reset" quiet size={28} onclick={() => focus.reset()} />
  </div>
</div>

<div class="ring" class:running={focus.running}>
  <Gauge value={1 - left / LENGTH[focus.kind]} size={104} stroke={5} accent={ACCENT} glow={focus.running}>
    <IconButton
      icon={focus.running ? 'pause' : 'play'}
      filled
      primary
      size={52}
      accent={ACCENT}
      label={focus.running ? 'Pause' : 'Start'}
      onclick={() => focus.toggle()}
    />
  </Gauge>
</div>

<style>
  .text {
    position: absolute;
    left: var(--edge);
    top: 40px;
    display: flex;
    flex-direction: column;
  }
  .time {
    font-family: var(--font-display);
    font-size: 48px;
    line-height: 56px;
    font-weight: 600;
    letter-spacing: -1px;
  }
  .caption {
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .row {
    margin-top: 14px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ring {
    position: absolute;
    right: var(--edge);
    top: 42px;
  }
  /* A slow breath while the timer runs. */
  .ring.running {
    animation: breathe 3.2s ease-in-out infinite;
  }
  @keyframes breathe {
    50% {
      transform: scale(1.025);
    }
  }
  /* On a side the ring sits on top, the time centred under it. */
  :global(.side) .ring {
    right: auto;
    left: 50%;
    translate: -50% 0;
    top: 76px;
  }
  :global(.side) .text {
    left: 0;
    right: 0;
    top: 196px;
    align-items: center;
  }
</style>
