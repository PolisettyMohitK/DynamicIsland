<script lang="ts">
  import { shelf, type Item } from '$lib/shelf.svelte';
  import Icon from '../Icon.svelte';

  const DRAG_PX = 5;

  // Press, then move a few px: drag the file out. Press and release: open it.
  let pressed: { item: Item; x: number; y: number } | null = null;

  function down(e: PointerEvent, item: Item) {
    if (e.button !== 0) return;
    pressed = { item, x: e.clientX, y: e.clientY };
  }

  function move(e: PointerEvent) {
    if (!pressed || Math.hypot(e.clientX - pressed.x, e.clientY - pressed.y) < DRAG_PX) return;
    shelf.dragOut(pressed.item.path);
    pressed = null;
  }

  function up() {
    if (pressed) shelf.open(pressed.item.path);
    pressed = null;
  }
</script>

{#if shelf.items.length}
  <div class="files" role="list" class:hovering={shelf.hovering} onpointermove={move} onpointerleave={() => (pressed = null)}>
    {#each shelf.items as item (item.path)}
      <div class="file" role="listitem" title={item.path}>
        <button class="tile" onpointerdown={(e) => down(e, item)} onpointerup={up}>
          {#if shelf.thumbs[item.path]}
            <img src={shelf.thumbs[item.path]} alt="" draggable="false" />
          {:else}
            <span class="ext"><Icon name="file" size={22} /><small>{item.ext.slice(0, 4) || 'file'}</small></span>
          {/if}
        </button>
        <span class="name">{item.name}</span>
        <button class="remove" aria-label="Remove from shelf" onclick={() => shelf.remove(item.path)}>
          <Icon name="x" size={10} />
        </button>
      </div>
    {/each}
  </div>
  <button class="clear" onclick={() => shelf.clear()}>Clear</button>
{:else}
  <div class="zone" class:hovering={shelf.hovering}>
    <span class="zone-icon"><Icon name="tray" size={22} /></span>
    <b>Drop files here</b>
    <span>Drag them back out anytime</span>
  </div>
{/if}

<style>
  .zone {
    position: absolute;
    left: var(--edge);
    right: var(--edge);
    top: 44px;
    height: 108px;
    border: 1.5px dashed rgba(255, 255, 255, 0.16);
    border-radius: 16px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    color: var(--text-dim);
    font-size: 12px;
    transition:
      border-color 150ms,
      background 150ms;
  }
  .zone b {
    color: var(--text);
    font-size: 13px;
    font-weight: 600;
  }
  .zone-icon {
    margin-bottom: 4px;
    transition: transform 360ms var(--spring);
  }
  .zone.hovering .zone-icon {
    transform: translateY(-4px) scale(1.12);
  }
  .zone.hovering,
  .files.hovering {
    border-color: rgba(255, 255, 255, 0.6);
    background: rgba(255, 255, 255, 0.06);
  }

  .files {
    position: absolute;
    left: var(--edge);
    right: var(--edge);
    top: 44px;
    height: 108px;
    padding: 10px 6px 0;
    border: 1.5px dashed transparent;
    border-radius: 16px;
    display: flex;
    gap: 6px;
    overflow-x: auto;
    scrollbar-width: none;
    transition:
      border-color 150ms,
      background 150ms;
  }
  .file {
    position: relative;
    width: 70px;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .tile {
    width: 60px;
    height: 60px;
    padding: 0;
    border: none;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.07);
    box-shadow: inset 0 0 0 1px var(--hairline);
    color: var(--text);
    overflow: hidden;
    cursor: grab;
    display: grid;
    place-items: center;
    animation: arrive 340ms var(--spring);
    transition: transform 260ms var(--spring);
  }
  @keyframes arrive {
    from {
      transform: scale(0.6);
      opacity: 0;
    }
  }
  .tile:hover {
    transform: translateY(-2px) scale(1.04);
  }
  .tile:active {
    transform: scale(0.92);
    transition-duration: 80ms;
  }
  .tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .ext {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .ext small {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .name {
    width: 100%;
    font-size: 11px;
    text-align: center;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .remove {
    position: absolute;
    top: -4px;
    right: 0;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgba(40, 40, 44, 0.95);
    color: var(--text);
    opacity: 0;
    cursor: pointer;
    transition: opacity 120ms;
  }
  .file:hover .remove {
    opacity: 1;
  }
  .remove:hover {
    background: #ff5a5f;
  }
  .clear {
    position: absolute;
    right: calc(var(--edge) + 2px);
    top: 154px;
    height: 20px;
    padding: 0 10px;
    border: none;
    border-radius: 10px;
    background: var(--pill);
    color: var(--text-dim);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }
  .clear:hover {
    background: var(--pill-hover);
    color: var(--text);
  }
  /* On a side the shelf is tall: files wrap into rows and scroll down. */
  :global(.side) .zone,
  :global(.side) .files {
    top: 72px;
    height: 228px;
  }
  :global(.side) .files {
    flex-wrap: wrap;
    align-content: flex-start;
    row-gap: 10px;
    overflow-x: hidden;
    overflow-y: auto;
  }
  :global(.side) .clear {
    top: 306px;
  }
</style>
