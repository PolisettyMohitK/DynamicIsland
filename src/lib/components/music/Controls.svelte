<script lang="ts">
  import { nowPlaying } from '$lib/media.svelte';
  import { status } from '$lib/status.svelte';
  import Icon from '../Icon.svelte';

  let { lyricsOn, onLyrics }: { lyricsOn: boolean; onLyrics: () => void } = $props();

  let showVolume = $state(false);
  const volume = $derived(status.current?.volume ?? 0);
  const silent = $derived(!!status.current?.muted || volume === 0);
</script>

<div class="controls">
  <button class="side" class:on={lyricsOn} aria-label="Lyrics" aria-pressed={lyricsOn} onclick={onLyrics}>
    <Icon name="lyrics" size={17} />
  </button>
  {#if showVolume}
    <div class="volume">
      <button class="mute" aria-label={silent ? 'Unmute' : 'Mute'} onclick={() => status.toggleMute()}>
        <Icon name={silent ? 'mute' : 'volume'} size={16} />
      </button>
      <input
        type="range"
        min="0"
        max="100"
        value={volume}
        style="--fill: {volume}%"
        aria-label="Volume"
        oninput={(e) => status.setVolume(+e.currentTarget.value)}
      />
    </div>
  {:else}
    <div class="transport">
      <button aria-label="Previous" onclick={() => nowPlaying.previous()}><Icon name="previous" size={19} filled /></button>
      <button class="play" aria-label={nowPlaying.media?.playing ? 'Pause' : 'Play'} onclick={() => nowPlaying.togglePlay()}>
        {#key nowPlaying.media?.playing}
          <span class="glyph"><Icon name={nowPlaying.media?.playing ? 'pause' : 'play'} size={24} filled /></span>
        {/key}
      </button>
      <button aria-label="Next" onclick={() => nowPlaying.next()}><Icon name="next" size={19} filled /></button>
    </div>
  {/if}
  <button
    class="side right"
    class:on={showVolume}
    aria-label="Volume"
    aria-pressed={showVolume}
    onclick={() => (showVolume = !showVolume)}
  >
    <Icon name={silent ? 'mute' : 'volume'} size={17} />
  </button>
</div>

<style>
  .controls {
    position: relative;
    height: 36px;
    display: flex;
    align-items: center;
  }
  .transport,
  .volume {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 20px;
  }
  .volume {
    gap: 8px;
    width: 60%;
  }
  button {
    border: none;
    background: none;
    color: var(--text);
    padding: 4px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    cursor: pointer;
    transition:
      background 160ms,
      transform 260ms var(--spring),
      color 160ms;
  }
  button:hover {
    background: var(--pill);
    transform: scale(1.08);
  }
  button:active {
    transform: scale(0.85);
    transition-duration: 80ms;
  }
  .play {
    padding: 6px;
  }
  .glyph {
    display: grid;
    animation: pop 260ms var(--spring);
  }
  @keyframes pop {
    from {
      transform: scale(0.5);
      opacity: 0;
    }
  }
  .side {
    color: var(--text-dim);
    padding: 7px;
  }
  .side.right {
    margin-left: auto;
  }
  .side.on {
    color: var(--text);
    background: var(--pill-hover);
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
    height: 4px;
    border-radius: 2px;
    background: linear-gradient(to right, var(--text) var(--fill), var(--track) var(--fill));
  }
  input[type='range']::-webkit-slider-thumb {
    appearance: none;
    width: 12px;
    height: 12px;
    margin-top: -4px;
    border-radius: 50%;
    background: var(--text);
  }
</style>
