<script module lang="ts">
  /** The session last opened up, to come back to when the island reopens soon after closing. */
  const last = { picked: undefined as string | null | undefined };
</script>

<script lang="ts">
  // Control Centre > Claude: your recent Claude Code sessions and how each is getting on, what
  // it's doing now and how far down its task list it is. Anything more opens in Claude itself.
  import { claude, type ClaudeSession } from '$lib/claude.svelte';
  import { clock } from '$lib/clock.svelte';
  import { remembers, ui } from '$lib/ui.svelte';
  import Icon from '../Icon.svelte';

  const STATUS = { working: 'Working', waiting: 'Waiting', done: 'Done', stopped: 'Idle' } as const;
  const SHOWN = 6;

  /** The session you opened up (null for none); left alone, the latest one that's busy. */
  let picked = $state<string | null | undefined>(remembers() ? last.picked : undefined);
  $effect(() => {
    last.picked = picked;
  });

  const busy = (s: ClaudeSession) => s.status === 'working' || s.status === 'waiting';
  const open = $derived(picked === undefined ? claude.sessions.find(busy)?.id : picked);
  /** Opens in the desktop app if that's the pick and it runs this one; otherwise a terminal. */
  const inDesktop = (s: ClaudeSession) => ui.claudeOpen === 'desktop' && !!s.desktop;
  const name = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  const done = (s: ClaudeSession) => s.tasks.filter((t) => t.status === 'completed').length;

  function ago(ms: number) {
    const min = Math.max(0, Math.round((clock.now - ms) / 60_000));
    if (min < 1) return 'now';
    if (min < 60) return `${min}m`;
    if (min < 60 * 24) return `${Math.round(min / 60)}h`;
    return `${Math.round(min / 60 / 24)}d`;
  }
</script>

<div class="list">
  {#each claude.sessions.slice(0, SHOWN) as s (s.id)}
    <button class="row" class:on={open === s.id} onclick={() => (picked = open === s.id ? null : s.id)}>
      <i class="dot {s.status}" title={STATUS[s.status]}></i>
      <span class="title">{s.title}</span>
      <span class="meta">{busy(s) && s.step ? s.step : `${name(s.cwd)} · ${ago(s.updated)}`}</span>
    </button>
    {#if open === s.id}
      <div class="more">
        {#if s.tasks.length}
          <div class="bar"><i style="width: {(done(s) / s.tasks.length) * 100}%"></i></div>
          <ol class="tasks">
            {#each s.tasks as t, i (i)}
              <li class={t.status}>
                {#if t.status === 'completed'}<Icon name="check" size={11} />{:else}<i></i>{/if}
                <span>{t.text}</span>
              </li>
            {/each}
          </ol>
        {:else if s.reply}
          <p class="said">{s.reply}</p>
        {/if}
        <div class="acts">
          <span class="where">
            {s.tasks.length ? `${done(s)} of ${s.tasks.length} done` : STATUS[s.status]}
          </span>
          <!-- Only a second terminal on a busy session would tangle; the desktop app just shows it. -->
          <button class="open" disabled={!inDesktop(s) && (busy(s) || !s.cwd)} onclick={() => claude.open(s, inDesktop(s))}>
            {inDesktop(s) ? 'Open in Claude' : 'Open in terminal'}
          </button>
        </div>
      </div>
    {/if}
  {:else}
    <p class="empty">Your Claude Code sessions show up here.</p>
  {/each}
</div>

<style>
  button {
    font: inherit;
    color: inherit;
    border: none;
    cursor: pointer;
  }
  .list {
    max-height: 300px;
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
  .more {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px 14px 6px 31px;
  }
  .bar {
    height: 3px;
    border-radius: 2px;
    background: var(--pill);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: #d97757;
    transition: width 400ms var(--ease-out);
  }
  .tasks {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 150px;
    overflow-y: auto;
    scrollbar-width: none;
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12px;
    line-height: 1.35;
  }
  .tasks li {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    color: var(--text-dim);
  }
  /* The tick, or a ring for what's to come (filled for the one under way). */
  .tasks li > :global(svg),
  .tasks li > i {
    flex: none;
    width: 11px;
    height: 11px;
    margin-top: 2px;
  }
  .tasks li > i {
    box-sizing: border-box;
    border-radius: 50%;
    border: 1.5px solid var(--text-faint);
  }
  .tasks .completed {
    color: var(--text-faint);
  }
  .tasks .completed > :global(svg) {
    color: var(--ok);
  }
  .tasks .in_progress {
    color: var(--text);
    font-weight: 600;
  }
  .tasks .in_progress > i {
    border: none;
    background: #d97757;
    scale: 0.7;
    animation: pulse 1.2s var(--ease-out) infinite;
  }
  .said {
    margin: 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--text-dim);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    overflow: hidden;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .where {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .open {
    flex: none;
    height: 24px;
    padding: 0 10px;
    border-radius: 12px;
    font-size: 11.5px;
    font-weight: 600;
    background: var(--pill);
    color: var(--text-dim);
  }
  .open:not(:disabled):hover {
    color: var(--text);
    background: var(--pill-hover);
  }
  .open:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .empty {
    margin: 0;
    padding: 24px 0;
    text-align: center;
    font-size: 12.5px;
    color: var(--text-dim);
  }
</style>
