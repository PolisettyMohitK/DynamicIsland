<script lang="ts">
  import { nowPlaying } from '$lib/media.svelte';
  import Artwork from './Artwork.svelte';
  import Controls from './Controls.svelte';
  import Progress from './Progress.svelte';

  let { onLyrics }: { onLyrics: () => void } = $props();

  const LINE = 28; // px per lyric line
  const VIEW = 180; // height of the lyrics column
  const offset = $derived(VIEW / 2 - LINE / 2 - Math.max(nowPlaying.line, 0) * LINE);
</script>

<div class="player">
  <div class="top">
    <Artwork size={56} round />
    <div class="meta">
      <b>{nowPlaying.media?.title}</b>
      <span>{nowPlaying.media?.artist}</span>
    </div>
  </div>
  <div class="progress"><Progress /></div>
  <div class="controls"><Controls lyricsOn onLyrics={onLyrics} /></div>
</div>

<div class="divider"></div>

<div class="lyrics">
  {#if nowPlaying.lyricsState === 'ready'}
    <ol style="transform: translateY({offset}px)">
      {#each nowPlaying.lyrics as l, i (i)}
        <li class:current={i === nowPlaying.line} class:past={i < nowPlaying.line}>
          <button onclick={() => nowPlaying.seek(l.t)}>{l.text}</button>
        </li>
      {/each}
    </ol>
  {:else}
    <p class="note">{nowPlaying.lyricsState === 'loading' ? 'Loading lyrics…' : 'Lyrics not available'}</p>
  {/if}
</div>

<style>
  .player {
    position: absolute;
    left: 28px;
    top: 0;
    width: 290px;
    height: 180px;
  }
  .top {
    position: absolute;
    top: 18px;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .meta {
    min-width: 0;
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
    font-size: 16px;
  }
  .meta span {
    font-size: 13px;
    color: var(--text-dim);
  }
  .progress {
    position: absolute;
    left: 0;
    right: 0;
    top: 90px;
  }
  .controls {
    position: absolute;
    left: 0;
    right: 0;
    top: 114px;
  }

  .divider {
    position: absolute;
    left: 336px;
    top: 24px;
    bottom: 24px;
    width: 1px;
    background: rgba(255, 255, 255, 0.12);
  }

  .lyrics {
    position: absolute;
    left: 352px;
    right: 24px;
    top: 0;
    height: 180px;
    overflow: hidden;
    /* Fade lines out towards the top and bottom edges. */
    mask: linear-gradient(transparent, #000 30%, #000 70%, transparent);
  }
  /* Docked on a side: the lyrics sit under the player instead of beside it. */
  :global(.side) .player {
    right: 28px;
    width: auto;
  }
  :global(.side) .divider {
    left: 28px;
    right: 28px;
    top: 180px;
    bottom: auto;
    width: auto;
    height: 1px;
  }
  :global(.side) .lyrics {
    left: 28px;
    right: 28px;
    top: 180px;
  }
  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    transition: transform 450ms var(--ease-out);
  }
  li {
    height: 28px;
    display: flex;
    align-items: center;
  }
  li button {
    max-width: 100%;
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    font-size: 15px;
    color: var(--text-dim);
    opacity: 0.7;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
    transition:
      color 300ms,
      opacity 300ms,
      transform 300ms var(--ease-out);
    transform-origin: left center;
  }
  li.past button {
    opacity: 0.45;
  }
  li.current button {
    color: var(--text);
    opacity: 1;
    font-weight: 600;
    transform: scale(1.04);
  }
  li button:hover {
    opacity: 1;
  }
  .note {
    margin: 0;
    height: 100%;
    display: flex;
    align-items: center;
    font-size: 14px;
    color: var(--text-dim);
  }
</style>
