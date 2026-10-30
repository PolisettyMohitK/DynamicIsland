<script lang="ts">
  // A small live card under Choco's reply: tasks, water, the timer or what's playing.
  import { clock } from '$lib/clock.svelte';
  import type { Panel } from '$lib/commands';
  import { focus, LABEL } from '$lib/focus.svelte';
  import { nowPlaying } from '$lib/media.svelte';
  import { mmss } from '$lib/store';
  import { todos } from '$lib/todos.svelte';
  import { water } from '$lib/water.svelte';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';

  let { panel }: { panel: Panel } = $props();

  const MAX_TASKS = 5;
  const open = $derived(todos.tasks.filter((t) => !t.doneOn));
</script>

<div class="card">
  {#if panel === 'tasks'}
    {#if todos.tasks.length === 0}
      <span class="empty">Nothing on your list today.</span>
    {:else}
      <ul>
        {#each todos.tasks.slice(0, MAX_TASKS) as t (t.id)}
          <li class:done={t.doneOn}>
            <button class="tick" aria-label={t.doneOn ? 'Mark not done' : 'Mark done'} onclick={() => todos.toggle(t.id)}>
              {#if t.doneOn}<Icon name="check" size={11} />{/if}
            </button>
            <span class="task">{t.text}</span>
            {#if t.due}<span class="due">{t.due}</span>{/if}
          </li>
        {/each}
      </ul>
      {#if todos.tasks.length > MAX_TASKS}<span class="more">{todos.tasks.length - MAX_TASKS} more</span>{/if}
      <span class="foot">{open.length} open · {todos.done} done</span>
    {/if}
  {:else if panel === 'water'}
    <div class="row">
      <Icon name="drop" size={15} />
      <b>{(water.ml / 1000).toFixed(2)} L</b><span class="dim fill">of {(water.goalMl / 1000).toFixed(1)} L</span>
      <IconButton icon="plus" size={26} label="Log a cup" accent="var(--accent-water)" onclick={() => water.drink()} />
    </div>
    <span class="bar"><i style="width: {water.percent}%; background: var(--accent-water)"></i></span>
  {:else if panel === 'timer'}
    <div class="row">
      <Icon name="timer" size={15} />
      <span class="dim fill">{LABEL[focus.kind]}</span>
      <b class="count">{mmss(Math.ceil(focus.remaining(clock.now) / 1000) * 1000)}</b>
      <IconButton icon={focus.running ? 'pause' : 'play'} size={26} filled label={focus.running ? 'Pause' : 'Start'} onclick={() => focus.toggle()} />
    </div>
  {:else if nowPlaying.media}
    <div class="row">
      {#if nowPlaying.artwork}<img src={nowPlaying.artwork} alt="" />{:else}<Icon name="music" size={15} />{/if}
      <span class="track"><b>{nowPlaying.media.title}</b><span class="dim">{nowPlaying.media.artist}</span></span>
      <IconButton icon="previous" size={26} quiet filled label="Previous" onclick={() => nowPlaying.previous()} />
      <IconButton icon={nowPlaying.media.playing ? 'pause' : 'play'} size={26} filled label="Play or pause" onclick={() => nowPlaying.togglePlay()} />
      <IconButton icon="next" size={26} quiet filled label="Next" onclick={() => nowPlaying.next()} />
    </div>
  {:else}
    <span class="empty">Nothing is playing.</span>
  {/if}
</div>

<style>
  .card {
    margin-top: 12px;
    padding: 10px 12px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px var(--hairline);
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12.5px;
    animation: rise 420ms var(--ease-out) both;
    animation-delay: 120ms;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .dim,
  .empty,
  .more,
  .foot,
  .due {
    color: var(--text-dim);
  }
  .foot,
  .more {
    font-size: 11.5px;
  }
  b {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .fill {
    flex: 1;
  }
  .count {
    font-size: 15px;
  }
  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--track);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    border-radius: 2px;
    transition: width 600ms var(--ease-out);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  li.done .task {
    color: var(--text-faint);
    text-decoration: line-through;
  }
  .task {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tick {
    width: 16px;
    height: 16px;
    flex: none;
    padding: 0;
    border: 1.5px solid var(--text-faint);
    border-radius: 50%;
    background: none;
    color: #1a1a1e;
    display: grid;
    place-items: center;
    cursor: pointer;
    transition:
      background 160ms,
      border-color 160ms;
  }
  li.done .tick {
    background: var(--text);
    border-color: var(--text);
  }
  img {
    width: 28px;
    height: 28px;
    border-radius: 7px;
    object-fit: cover;
  }
  .track {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .track > * {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
