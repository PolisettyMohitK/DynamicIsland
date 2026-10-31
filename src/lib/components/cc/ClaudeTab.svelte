<script lang="ts">
  // Control Centre > Claude: your recent Claude Code sessions, what each is doing, and a box to
  // send a prompt, continuing the picked session or starting one in a recent folder.
  import { claude, type ClaudeSession } from '$lib/claude.svelte';
  import { clock } from '$lib/clock.svelte';
  import { startTyping, stopTyping } from '$lib/ui.svelte';
  import Icon from '../Icon.svelte';

  const STATUS = { working: 'Working', waiting: 'Waiting', done: 'Done', stopped: 'Idle' } as const;
  const SHOWN = 6;

  let picked = $state<string | null>(null);
  let text = $state('');
  let field = $state<HTMLInputElement>();
  let folderAt = $state(0);

  const selected = $derived(claude.sessions.find((s) => s.id === picked));
  /** New sessions start in one of the recent sessions' folders. */
  const folders = $derived([...new Set(claude.sessions.map((s) => s.cwd).filter(Boolean))]);
  const folder = $derived(folders.length ? folders[folderAt % folders.length] : '');
  /** Running somewhere else; a second prompt into it would tangle the two. */
  const busy = (s: ClaudeSession) => s.status === 'working' || s.status === 'waiting';
  const blocked = $derived(claude.sending || (selected ? busy(selected) : !folder));
  const name = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

  function ago(ms: number) {
    const min = Math.max(0, Math.round((clock.now - ms) / 60_000));
    if (min < 1) return 'now';
    if (min < 60) return `${min}m`;
    if (min < 60 * 24) return `${Math.round(min / 60)}h`;
    return `${Math.round(min / 60 / 24)}d`;
  }

  function send(e: SubmitEvent) {
    e.preventDefault();
    const prompt = text.trim();
    if (!prompt || blocked) return;
    if (selected) claude.send(prompt, selected.cwd, selected.id);
    else claude.send(prompt, folder);
    text = '';
    stopTyping();
  }

  $effect(() => () => stopTyping());
</script>

<div class="list">
  {#each claude.sessions.slice(0, SHOWN) as s (s.id)}
    <button class="row" class:on={picked === s.id} onclick={() => (picked = picked === s.id ? null : s.id)}>
      <i class="dot {s.status}" title={STATUS[s.status]}></i>
      <span class="title">{s.title}</span>
      <span class="meta">{name(s.cwd)} · {ago(s.updated)}</span>
    </button>
    {#if picked === s.id && s.reply}
      <p class="said">{s.reply}</p>
    {/if}
  {:else}
    <p class="empty">Your Claude Code sessions show up here.</p>
  {/each}
</div>

{#if claude.reply}
  <div class="reply" class:bad={!claude.reply.ok}>
    <p>{claude.reply.text || 'Done, nothing to say.'}</p>
    <button class="close" aria-label="Dismiss" onclick={() => (claude.reply = null)}><Icon name="x" size={13} /></button>
  </div>
{/if}

<form class="ask" onsubmit={send}>
  {#if selected}
    <span class="where" title={selected.cwd}>Continue</span>
  {:else}
    <button type="button" class="where pick" title={folder} disabled={folders.length < 2} onclick={() => folderAt++}>
      New in {folder ? name(folder) : '…'}
    </button>
  {/if}
  <input
    bind:this={field}
    bind:value={text}
    placeholder={claude.sending
      ? 'Claude is on it…'
      : selected && busy(selected)
        ? 'Busy in another window'
        : selected
          ? `Ask "${selected.title}"`
          : 'Ask Claude Code'}
    disabled={blocked}
    onclick={() => field && startTyping(field)}
    onkeydown={(e) => e.key === 'Escape' && stopTyping()}
  />
  <button type="submit" class="send" aria-label="Send" disabled={blocked || !text.trim()}><Icon name="sparkle" size={14} /></button>
</form>
<p class="hint">Runs in the background. It can read and answer, but won't edit files or run commands.</p>

<style>
  button {
    font: inherit;
    color: inherit;
    border: none;
    cursor: pointer;
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
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12.5px;
    text-align: left;
    transition:
      background 160ms,
      transform 160ms var(--ease-out);
  }
  .row:hover,
  .row.on {
    background: var(--pill-hover);
  }
  .row:active {
    transform: scale(0.98);
  }
  .title {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    flex: none;
    max-width: 40%;
    font-size: 11.5px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .dot.working {
    background: #d97757;
    animation: pulse 1.2s var(--ease-out) infinite;
  }
  .dot.waiting {
    background: #f2c14e;
  }
  .dot.done {
    background: var(--ok);
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .said {
    margin: 0;
    padding: 2px 14px 6px 31px;
    font-size: 12px;
    line-height: 1.45;
    color: var(--text-dim);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .empty {
    margin: 0;
    padding: 24px 0;
    text-align: center;
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .reply {
    position: relative;
    padding: 10px 34px 10px 14px;
    border-radius: 12px;
    background: color-mix(in srgb, #d97757 14%, transparent);
  }
  .reply.bad {
    background: color-mix(in srgb, var(--bad) 14%, transparent);
  }
  .reply p {
    margin: 0;
    max-height: 96px;
    overflow-y: auto;
    scrollbar-width: none;
    font-size: 12.5px;
    line-height: 1.45;
    white-space: pre-wrap;
  }
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 22px;
    height: 22px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    background: none;
    color: var(--text-dim);
  }
  .close:hover {
    color: var(--text);
  }
  .ask {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 36px;
    padding: 0 4px 0 6px;
    border-radius: 18px;
    background: var(--pill);
  }
  .where {
    flex: none;
    max-width: 140px;
    height: 26px;
    padding: 0 10px;
    border-radius: 13px;
    display: inline-flex;
    align-items: center;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    background: var(--pill);
  }
  .pick:not(:disabled):hover {
    color: var(--text);
    background: var(--pill-hover);
  }
  .pick:disabled {
    cursor: default;
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: none;
    outline: none;
    background: none;
    color: var(--text);
    font: 500 12.5px var(--font);
  }
  input::placeholder {
    color: var(--text-faint);
  }
  .send {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    background: #d97757;
    color: #fff;
    transition: opacity 160ms;
  }
  .send:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .hint {
    margin: 0;
    text-align: center;
    font-size: 11px;
    color: var(--text-faint);
  }
</style>
