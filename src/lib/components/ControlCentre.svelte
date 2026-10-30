<script lang="ts">
  import { fade } from 'svelte/transition';
  import { control, type PowerAction } from '$lib/control.svelte';
  import { nowPlaying } from '$lib/media.svelte';
  import { status } from '$lib/status.svelte';
  import type { IconName } from '$lib/icons';
  import Icon from './Icon.svelte';
  import Artwork from './music/Artwork.svelte';
  import AppsTab from './cc/AppsTab.svelte';
  import ClipboardTab from './cc/ClipboardTab.svelte';
  import StatsTab from './cc/StatsTab.svelte';
  import TodayTab from './cc/TodayTab.svelte';
  import WallpaperTab from './cc/WallpaperTab.svelte';

  const TABS = ['Controls', 'Today', 'Stats', 'Clipboard', 'Apps', 'Wallpaper', 'Power'] as const;
  const POWER: { action: PowerAction; label: string; icon: IconName; confirm?: boolean }[] = [
    { action: 'lock', label: 'Lock', icon: 'lock' },
    { action: 'sleep', label: 'Sleep', icon: 'moon' },
    { action: 'signout', label: 'Sign out', icon: 'logout', confirm: true },
    { action: 'restart', label: 'Restart', icon: 'reset', confirm: true },
    { action: 'shutdown', label: 'Shut down', icon: 'power', confirm: true },
  ];
  /** How long a "click again" stays armed. */
  const ARM_MS = 3000;

  let tab = $state<(typeof TABS)[number]>('Controls');
  let picking = $state(false);
  let armed = $state<PowerAction | null>(null);
  let disarm: ReturnType<typeof setTimeout>;

  const s = $derived(control.state);
  const output = $derived(s?.outputs.find((o) => o.default));
  const volume = $derived(status.current?.volume ?? 0);
  const silent = $derived(!!status.current?.muted || volume === 0);

  $effect(() => {
    control.refresh();
    return () => clearTimeout(disarm);
  });

  // Anything that ends a session asks for a second click first.
  function press(p: (typeof POWER)[number]) {
    if (p.confirm && armed !== p.action) {
      armed = p.action;
      clearTimeout(disarm);
      disarm = setTimeout(() => (armed = null), ARM_MS);
      return;
    }
    armed = null;
    control.power(p.action);
  }
</script>

<div class="cc">
  <nav>
    {#each TABS as t}
      <button class="tab" class:on={tab === t} onclick={() => ((tab = t), (picking = false))}>{t}</button>
    {/each}
  </nav>

  {#if tab === 'Controls'}
    <div class="body" in:fade={{ duration: 160 }}>
      {#if picking}
        <div class="list">
          {#each s?.outputs ?? [] as o (o.id)}
            <button class="row" class:on={o.default} onclick={() => ((picking = false), control.setOutput(o.id))}>
              <Icon name={/head|ear|bud|pod/i.test(o.name) ? 'headphones' : 'speaker'} size={16} />
              <span>{o.name}</span>
              {#if o.default}<Icon name="check" size={16} />{/if}
            </button>
          {/each}
        </div>
      {:else}
        <div class="tiles three">
          <button
            class="tile"
            class:on={s?.wifi}
            disabled={s?.wifi == null}
            onclick={() => control.setRadio('wifi', !s?.wifi)}
          >
            <span class="dot"><Icon name="wifi" size={16} /></span>
            <span class="text"><b>Wi-Fi</b><small>{s?.wifi ? (status.current?.network?.wifi ? status.current.network.name : 'On') : 'Off'}</small></span>
          </button>
          <button
            class="tile"
            class:on={s?.bluetooth}
            disabled={s?.bluetooth == null}
            onclick={() => control.setRadio('bluetooth', !s?.bluetooth)}
          >
            <span class="dot"><Icon name="bluetooth" size={16} /></span>
            <span class="text"><b>Bluetooth</b><small>{s?.bluetooth ? 'On' : 'Off'}</small></span>
          </button>
          <button class="tile on" disabled={!s?.outputs.length} onclick={() => (picking = true)}>
            <span class="dot"><Icon name={/head|ear|bud|pod/i.test(output?.name ?? '') ? 'headphones' : 'speaker'} size={16} /></span>
            <span class="text"><b>Output</b><small>{output?.name ?? '—'}</small></span>
            <span class="more"><Icon name="chevron-down" size={14} /></span>
          </button>
        </div>
        <div class="tiles two">
          <button class="tile" class:on={s?.dark} onclick={() => control.setDark(!s?.dark)}>
            <span class="dot"><Icon name="contrast" size={16} /></span>
            <span class="text"><b>Dark mode</b><small>{s?.dark ? 'On' : 'Off'}</small></span>
          </button>
          <button class="tile" class:on={!silent} onclick={() => status.toggleMute()}>
            <span class="dot"><Icon name={silent ? 'mute' : 'volume'} size={16} /></span>
            <span class="text"><b>Sound</b><small>{silent ? 'Muted' : 'On'}</small></span>
          </button>
        </div>

        <label class="slider">
          <Icon name={silent ? 'mute' : 'volume'} size={16} />
          <input
            type="range"
            min="0"
            max="100"
            value={volume}
            style="--fill: {volume}%"
            aria-label="Volume"
            oninput={(e) => status.setVolume(+e.currentTarget.value)}
          />
          <span class="num">{volume}</span>
        </label>
        {#if s?.brightness != null}
          <label class="slider">
            <Icon name="sun" size={16} />
            <input
              type="range"
              min="0"
              max="100"
              value={s.brightness}
              style="--fill: {s.brightness}%"
              aria-label="Brightness"
              oninput={(e) => control.setBrightness(+e.currentTarget.value)}
            />
            <span class="num">{s.brightness}</span>
          </label>
        {/if}

        {#if nowPlaying.media}
          <div class="media">
            <Artwork size={34} />
            <span class="text"><b>{nowPlaying.media.title}</b><small>{nowPlaying.media.artist}</small></span>
            <button class="icon" aria-label="Previous" onclick={() => nowPlaying.previous()}><Icon name="previous" size={16} filled /></button>
            <button class="icon" aria-label={nowPlaying.media.playing ? 'Pause' : 'Play'} onclick={() => nowPlaying.togglePlay()}>
              <Icon name={nowPlaying.media.playing ? 'pause' : 'play'} size={18} filled />
            </button>
            <button class="icon" aria-label="Next" onclick={() => nowPlaying.next()}><Icon name="next" size={16} filled /></button>
          </div>
        {/if}
      {/if}
    </div>
  {:else if tab === 'Today'}
    <div class="body" in:fade={{ duration: 160 }}><TodayTab /></div>
  {:else if tab === 'Wallpaper'}
    <div class="body" in:fade={{ duration: 160 }}><WallpaperTab /></div>
  {:else if tab === 'Stats'}
    <div class="body" in:fade={{ duration: 160 }}><StatsTab /></div>
  {:else if tab === 'Clipboard'}
    <div class="body" in:fade={{ duration: 160 }}><ClipboardTab /></div>
  {:else if tab === 'Apps'}
    <div class="body" in:fade={{ duration: 160 }}><AppsTab /></div>
  {:else}
    <div class="body" in:fade={{ duration: 160 }}>
      <div class="tiles five">
        {#each POWER as p (p.action)}
          <button class="power" class:armed={armed === p.action} onclick={() => press(p)}>
            <span class="dot"><Icon name={p.icon} size={18} /></span>
            <span>{armed === p.action ? 'Click again' : p.label}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}

  {#if control.error}
    <p class="error">{control.error}</p>
  {/if}
</div>

<style>
  .cc {
    width: 560px;
    padding: 12px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  nav {
    display: flex;
    justify-content: center;
    gap: 4px;
  }
  .tab {
    height: 26px;
    padding: 0 12px;
    border: none;
    border-radius: 13px;
    background: none;
    color: var(--text-faint);
    font: 600 12px var(--font);
    cursor: pointer;
    transition:
      color 160ms,
      background 160ms;
  }
  .tab:hover {
    color: var(--text-dim);
  }
  .tab.on {
    color: var(--text);
    background: var(--pill-hover);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .tiles {
    display: grid;
    gap: 8px;
  }
  .three {
    grid-template-columns: repeat(3, 1fr);
  }
  .two {
    grid-template-columns: repeat(2, 1fr);
  }
  .five {
    grid-template-columns: repeat(5, 1fr);
  }
  button {
    font: inherit;
    color: inherit;
    border: none;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .tile {
    position: relative;
    height: 52px;
    padding: 0 10px;
    border-radius: 18px;
    background: var(--pill);
    display: flex;
    align-items: center;
    gap: 10px;
    text-align: left;
    transition:
      background 160ms,
      transform 160ms var(--ease-out);
  }
  .tile:not(:disabled):hover {
    background: var(--pill-hover);
  }
  .tile:not(:disabled):active,
  .power:active {
    transform: scale(0.96);
  }
  /* The switch's state lives in its round icon: lit white when on, like iOS. */
  .dot {
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--pill-hover);
    color: var(--text);
    transition:
      background 200ms,
      color 200ms;
  }
  .tile.on .dot {
    background: var(--text);
    color: #111;
  }
  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    line-height: 1.25;
  }
  .text b {
    font-size: 12.5px;
    font-weight: 600;
  }
  .text small,
  .text b {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .text small {
    font-size: 11.5px;
    color: var(--text-dim);
  }
  .more {
    margin-left: auto;
    color: var(--text-faint);
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row {
    height: 38px;
    padding: 0 12px;
    border-radius: 12px;
    background: none;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: var(--text-dim);
    text-align: left;
  }
  .row span {
    flex: 1;
  }
  .row:hover {
    background: var(--pill);
  }
  .row.on {
    color: var(--text);
  }

  .slider {
    height: 36px;
    padding: 0 14px;
    border-radius: 18px;
    background: var(--pill);
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-dim);
  }
  .num {
    width: 24px;
    text-align: right;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  input[type='range'] {
    flex: 1;
    height: 16px;
    margin: 0;
    background: none;
    appearance: none;
    cursor: pointer;
  }
  input[type='range']::-webkit-slider-runnable-track {
    height: 6px;
    border-radius: 3px;
    background: linear-gradient(to right, var(--text) var(--fill), var(--track) var(--fill));
  }
  input[type='range']::-webkit-slider-thumb {
    appearance: none;
    width: 14px;
    height: 14px;
    margin-top: -4px;
    border-radius: 50%;
    background: var(--text);
  }

  .media {
    height: 50px;
    padding: 0 8px;
    border-radius: 18px;
    background: var(--pill);
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .media .text {
    flex: 1;
  }
  .icon {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: none;
    display: grid;
    place-items: center;
    transition:
      background 160ms,
      transform 160ms var(--ease-out);
  }
  .icon:hover {
    background: var(--pill-hover);
  }
  .icon:active {
    transform: scale(0.96);
  }

  .power {
    height: 84px;
    border-radius: 18px;
    background: var(--pill);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    font-size: 12px;
    font-weight: 600;
    transition:
      background 160ms,
      transform 160ms var(--ease-out);
  }
  .power .dot {
    width: 38px;
    height: 38px;
  }
  .power:hover {
    background: var(--pill-hover);
  }
  .power.armed {
    color: var(--bad);
  }
  .power.armed .dot {
    background: var(--bad);
    color: #111;
  }
  .error {
    margin: 0;
    font-size: 11.5px;
    color: var(--bad);
    text-align: center;
  }
</style>
