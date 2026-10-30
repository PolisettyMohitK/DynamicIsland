<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { load, save } from '$lib/store';
  import { startTyping, stopTyping } from '$lib/ui.svelte';
  import Icon from '../Icon.svelte';

  type App = { name: string; id: string };
  const RECENT = 6;
  const SHOWN = 12;

  let apps = $state<App[]>([]);
  let error = $state('');
  let query = $state('');
  let recent = $state<string[]>(load('recent-apps', []));
  let field: HTMLInputElement | undefined = $state();

  $effect(() => {
    invoke<App[]>('apps_list')
      .then((a) => (apps = a))
      .catch((e) => (error = String(e)));
    return () => stopTyping();
  });

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) {
      const byId = new Map(apps.map((a) => [a.id, a]));
      return recent.map((id) => byId.get(id)).filter((a): a is App => !!a);
    }
    // Names that start with the query first, then words that do, then anywhere.
    const rank = (n: string) => (n.startsWith(q) ? 0 : n.includes(' ' + q) ? 1 : n.includes(q) ? 2 : 3);
    return apps
      .map((a) => ({ a, r: rank(a.name.toLowerCase()) }))
      .filter((x) => x.r < 3)
      .sort((x, y) => x.r - y.r)
      .slice(0, SHOWN)
      .map((x) => x.a);
  });

  function launch(app: App) {
    recent = [app.id, ...recent.filter((id) => id !== app.id)].slice(0, RECENT);
    save('recent-apps', recent);
    query = '';
    stopTyping();
    invoke('apps_launch', { id: app.id }).catch((e) => (error = String(e)));
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && shown[0]) launch(shown[0]);
    if (e.key === 'Escape') {
      query = '';
      stopTyping();
    }
  }
</script>

<label class="search">
  <Icon name="search" size={16} />
  <input
    bind:this={field}
    bind:value={query}
    placeholder="Search apps"
    spellcheck="false"
    onpointerdown={() => field && startTyping(field)}
    onblur={(e) => !e.relatedTarget && stopTyping()}
    onkeydown={onKey}
  />
</label>

{#if shown.length}
  {#if !query.trim()}<span class="head">Recent</span>{/if}
  <div class="grid">
    {#each shown as app (app.id)}
      <button class="app" title={app.name} onclick={() => launch(app)}>
        <span class="avatar">{app.name.match(/[a-z0-9]/i)?.[0]?.toUpperCase() ?? '•'}</span>
        <span class="name">{app.name}</span>
      </button>
    {/each}
  </div>
{:else}
  <p class="empty">
    {#if error}{error}{:else if !apps.length}Reading the Start menu…{:else if query.trim()}No app called “{query.trim()}”{:else}Search for an app to open it.{/if}
  </p>
{/if}

<style>
  .search {
    height: 38px;
    padding: 0 14px;
    border-radius: 19px;
    background: var(--pill);
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-dim);
    cursor: text;
  }
  input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: none;
    color: var(--text);
    font: 13px var(--font);
  }
  input::placeholder {
    color: var(--text-faint);
  }
  .head {
    padding: 0 4px;
    font-size: 11.5px;
    color: var(--text-dim);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 6px;
  }
  .app {
    min-width: 0;
    padding: 10px 4px 8px;
    border: none;
    border-radius: 16px;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    transition:
      background 160ms,
      transform 160ms var(--ease-out);
  }
  .app:hover {
    background: var(--pill);
  }
  .app:active {
    transform: scale(0.96);
  }
  .avatar {
    width: 40px;
    height: 40px;
    border-radius: 12px;
    background: var(--pill-hover);
    display: grid;
    place-items: center;
    font-size: 17px;
    font-weight: 600;
  }
  .name {
    max-width: 100%;
    font-size: 11px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .empty {
    margin: 0;
    padding: 20px 0;
    text-align: center;
    font-size: 12.5px;
    color: var(--text-dim);
  }
</style>
