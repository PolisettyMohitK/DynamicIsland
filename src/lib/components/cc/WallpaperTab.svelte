<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Icon from '../Icon.svelte';

  type Wall = { path: string; current: boolean };

  let walls = $state<Wall[]>([]);
  let thumbs = $state<Record<string, string>>({});
  let error = $state('');
  let loaded = $state(false);

  $effect(() => {
    invoke<Wall[]>('wallpaper_list')
      .then(async (w) => {
        walls = w;
        loaded = true;
        // One at a time: the shell makes each thumbnail, and the list stays responsive.
        for (const { path } of w) {
          const t = await invoke<string>('wallpaper_thumb', { path }).catch(() => '');
          if (t) thumbs[path] = t;
        }
      })
      .catch((e) => (error = String(e)));
  });

  async function pick(w: Wall) {
    error = '';
    try {
      await invoke('wallpaper_set', { path: w.path });
      walls.forEach((x) => (x.current = x === w));
    } catch (e) {
      error = String(e);
    }
  }

  const name = (path: string) => path.split('\\').pop()?.replace(/\.[^.]+$/, '') ?? path;
</script>

<div class="top">
  <span>Your Pictures\Wallpapers folder, then the ones Windows ships</span>
  <button class="add" onclick={() => invoke('wallpaper_open_folder').catch((e) => (error = String(e)))}>
    <Icon name="plus" size={13} />Add your own
  </button>
</div>

{#if walls.length}
  <!-- A mouse wheel scrolls the strip sideways instead of closing the Control Centre. -->
  <div
    class="strip"
    onwheel={(e) => {
      e.stopPropagation();
      if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
        e.preventDefault();
        e.currentTarget.scrollBy({ left: e.deltaY, behavior: 'smooth' });
      }
    }}
  >
    {#each walls as w (w.path)}
      <button class="wall" class:on={w.current} title={name(w.path)} onclick={() => pick(w)}>
        {#if thumbs[w.path]}<img src={thumbs[w.path]} alt="" />{/if}
        {#if w.current}<span class="tick"><Icon name="check" size={12} /></span>{/if}
      </button>
    {/each}
  </div>
{:else}
  <p class="empty">{error || (loaded ? 'No images found.' : 'Looking for wallpapers…')}</p>
{/if}
{#if error && walls.length}<p class="error">{error}</p>{/if}

<style>
  .top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 4px;
    font-size: 11.5px;
    color: var(--text-dim);
  }
  .add {
    height: 24px;
    padding: 0 10px 0 8px;
    border: none;
    border-radius: 12px;
    background: var(--pill);
    color: var(--text);
    font: 600 11.5px var(--font);
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
  }
  .add:hover {
    background: var(--pill-hover);
  }
  .strip {
    display: grid;
    grid-auto-flow: column;
    grid-template-rows: repeat(2, 84px);
    grid-auto-columns: 140px;
    gap: 8px;
    overflow-x: auto;
    scrollbar-width: none;
    scroll-snap-type: x mandatory;
  }
  .wall {
    position: relative;
    padding: 0;
    border: none;
    border-radius: 14px;
    background: var(--pill);
    overflow: hidden;
    cursor: pointer;
    scroll-snap-align: start;
    outline: 2px solid transparent;
    outline-offset: -2px;
    transition:
      outline-color 160ms,
      transform 160ms var(--ease-out);
  }
  .wall:active {
    transform: scale(0.96);
  }
  .wall.on {
    outline-color: var(--text);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    animation: appear 300ms var(--ease-out);
  }
  @keyframes appear {
    from {
      opacity: 0;
    }
  }
  .tick {
    position: absolute;
    right: 6px;
    bottom: 6px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--text);
    color: #111;
    display: grid;
    place-items: center;
  }
  .empty {
    margin: 0;
    padding: 24px 0;
    text-align: center;
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .error {
    margin: 0;
    font-size: 11.5px;
    color: var(--bad);
    text-align: center;
  }
</style>
