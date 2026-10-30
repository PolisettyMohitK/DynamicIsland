<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';

  let items = $state<string[]>([]);
  let error = $state('');

  const refresh = async () => (items = await invoke<string[]>('clipboard_history').catch(() => items));

  $effect(() => {
    refresh();
    const off = listen('clipboard-changed', refresh);
    return () => off.then((f) => f());
  });

  async function paste(text: string) {
    error = '';
    await invoke('clipboard_paste', { text }).catch((e) => (error = String(e)));
  }

  async function clear() {
    await invoke('clipboard_clear');
    items = [];
  }
</script>

{#if items.length}
  <div class="top">
    <span>Click to paste into the app you're using</span>
    <button class="clear" onclick={clear}>Clear</button>
  </div>
  <div class="list">
    {#each items as text (text)}
      <button class="row" title={text.length > 300 ? text.slice(0, 300) + '…' : text} onclick={() => paste(text)}>
        {text.replace(/\s+/g, ' ').trim()}
      </button>
    {/each}
  </div>
{:else}
  <p class="empty">Things you copy show up here.<br /><small>Kept in memory only, never saved to disk.</small></p>
{/if}
{#if error}<p class="error">{error}</p>{/if}

<style>
  .top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 4px;
    font-size: 11.5px;
    color: var(--text-dim);
  }
  button {
    font: inherit;
    color: inherit;
    border: none;
    cursor: pointer;
  }
  .clear {
    height: 24px;
    padding: 0 10px;
    border-radius: 12px;
    background: var(--pill);
    font-size: 11.5px;
    font-weight: 600;
  }
  .clear:hover {
    background: var(--pill-hover);
  }
  .list {
    max-height: 236px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    scrollbar-width: none;
  }
  .row {
    flex: none;
    height: 36px;
    padding: 0 14px;
    border-radius: 12px;
    background: var(--pill);
    font-size: 12.5px;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition:
      background 160ms,
      transform 160ms var(--ease-out);
  }
  .row:hover {
    background: var(--pill-hover);
  }
  .row:active {
    transform: scale(0.98);
  }
  .empty {
    margin: 0;
    padding: 24px 0;
    text-align: center;
    font-size: 12.5px;
    line-height: 1.5;
  }
  .empty small {
    color: var(--text-dim);
  }
  .error {
    margin: 0;
    font-size: 11.5px;
    color: var(--bad);
    text-align: center;
  }
</style>
