<script lang="ts">
  import { hhmm } from '$lib/store';
  import { water } from '$lib/water.svelte';
  import Digits from '../Digits.svelte';
  import Icon from '../Icon.svelte';
  import IconButton from '../IconButton.svelte';
  import Pill from '../Pill.svelte';

  const ACCENT = 'var(--accent-water)';
  const litres = (ml: number) => (ml / 1000).toFixed(ml % 100 ? 2 : 1);

  const caption = $derived(
    water.ml >= water.goalMl
      ? 'Goal reached today'
      : water.reminder
        ? `Next reminder ${hhmm(water.nextAt)}`
        : 'Reminders off',
  );
</script>

<div class="text">
  <div class="amount"><b><Digits value={litres(water.ml)} /></b><span>/ {litres(water.goalMl)} L</span></div>
  <span class="caption">{caption}</span>
  <div class="row">
    <Pill primary accent={ACCENT} onclick={() => water.drink()}><Icon name="plus" size={14} />{water.cupMl} ml</Pill>
    <IconButton
      icon={water.reminder ? 'bell' : 'bell-off'}
      size={28}
      active={water.reminder}
      accent={ACCENT}
      label={water.reminder ? 'Turn reminders off' : 'Turn reminders on'}
      onclick={() => water.toggleReminder()}
    />
    {#if water.ml > 0}
      <IconButton icon="reset" size={28} quiet label="Remove one cup" onclick={() => water.undo()} />
    {/if}
  </div>
</div>

<!-- A glass of water: the level rises with the day's total, the surface keeps moving. -->
<div class="orb" style="--level: {water.percent}%">
  <div class="liquid">
    <svg class="wave" viewBox="0 0 200 20" preserveAspectRatio="none" aria-hidden="true">
      <path d="M0 10 Q25 2 50 10 T100 10 T150 10 T200 10 V20 H0z" />
    </svg>
  </div>
  <span class="pct"><Digits value={`${water.percent}%`} /></span>
</div>

<style>
  .text {
    position: absolute;
    left: var(--edge);
    top: 40px;
    display: flex;
    flex-direction: column;
  }
  .amount {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .amount b {
    font-family: var(--font-display);
    font-size: 48px;
    line-height: 56px;
    font-weight: 600;
    letter-spacing: -1px;
  }
  .amount span {
    font-size: 15px;
    color: var(--text-dim);
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

  .orb {
    position: absolute;
    right: var(--edge);
    top: 42px;
    width: 104px;
    height: 104px;
    border-radius: 50%;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.05);
    box-shadow:
      inset 0 0 0 1px var(--hairline),
      inset 0 8px 18px rgba(255, 255, 255, 0.04);
    display: grid;
    place-items: center;
  }
  .liquid {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--level);
    background: linear-gradient(to bottom, color-mix(in srgb, var(--accent-water) 85%, #fff), var(--accent-water));
    opacity: 0.85;
    transition: height 900ms var(--ease-out);
  }
  .wave {
    position: absolute;
    left: 0;
    bottom: 100%;
    width: 200%;
    height: 10px;
    margin-bottom: -1px;
    fill: color-mix(in srgb, var(--accent-water) 85%, #fff);
    animation: drift 3s linear infinite;
  }
  @keyframes drift {
    to {
      transform: translateX(-50%);
    }
  }
  .pct {
    position: relative;
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 600;
    text-shadow: 0 1px 6px rgba(0, 0, 0, 0.35);
  }
</style>
