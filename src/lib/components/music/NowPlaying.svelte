<script lang="ts">
  import { nowPlaying } from '$lib/media.svelte';
  import Bars from '../Bars.svelte';
  import Icon from '../Icon.svelte';
  import Artwork from './Artwork.svelte';
  import Controls from './Controls.svelte';
  import Progress from './Progress.svelte';
  import Waves from './Waves.svelte';
  import { ui } from '$lib/ui.svelte';

  let { lyricsOn, onLyrics }: { lyricsOn: boolean; onLyrics: () => void } = $props();
</script>

{#if nowPlaying.media}
  {#if nowPlaying.media.playing}
    <!-- Behind everything else, so the controls stay on top. -->
    <div class="waves"><Waves style={ui.wave} colour={nowPlaying.colour ?? '255 255 255'} /></div>
  {/if}
  <div class="art" class:paused={!nowPlaying.media.playing}><Artwork size={64} /></div>
  <div class="meta">
    <b>{nowPlaying.media.title}</b>
    <span>{nowPlaying.media.artist}</span>
  </div>
  <div class="eq">
    <Bars playing={nowPlaying.media.playing} colour={nowPlaying.colour ? `rgb(${nowPlaying.colour})` : 'var(--text-dim)'} size={14} />
  </div>
  <div class="progress"><Progress /></div>
  <div class="controls"><Controls {lyricsOn} {onLyrics} /></div>
{:else}
  <div class="empty">
    <Icon name="music" size={24} />
    <span>Nothing playing</span>
  </div>
{/if}

<style>
  .waves {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 80px;
    opacity: 0.45;
    pointer-events: none;
  }
  .art {
    position: absolute;
    left: var(--edge);
    top: 16px;
    border-radius: 12px;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.35);
    transition: transform 420ms var(--spring);
  }
  /* The artwork sinks back a little while paused. */
  .art.paused {
    transform: scale(0.9);
  }
  .meta {
    position: absolute;
    left: 120px;
    right: 72px;
    top: 44px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .meta b,
  .meta span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta b {
    font-size: 15px;
    font-weight: 600;
  }
  .meta span {
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .eq {
    position: absolute;
    right: var(--edge);
    top: 50px;
  }
  .progress {
    position: absolute;
    left: var(--edge);
    right: var(--edge);
    top: 92px;
  }
  .controls {
    position: absolute;
    left: calc(var(--edge) - 6px);
    right: calc(var(--edge) - 6px);
    top: 116px;
  }
  .empty {
    position: absolute;
    inset: 44px 52px 28px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--text-faint);
  }
  /* On a side the player stands: the artwork big on top, then the words, the bar and the
     buttons under it, all centred. */
  :global(.side) .art {
    left: 50%;
    translate: -50% 0;
    top: 92px;
    scale: 1.5;
  }
  :global(.side) .meta {
    left: var(--edge);
    right: var(--edge);
    top: 186px;
    align-items: center;
    text-align: center;
  }
  :global(.side) .meta b,
  :global(.side) .meta span {
    max-width: 100%;
  }
  :global(.side) .eq {
    top: 76px;
  }
  :global(.side) .progress {
    top: 238px;
  }
  :global(.side) .controls {
    top: 266px;
  }
  :global(.side) .empty {
    inset: 72px 28px 40px;
  }
</style>
