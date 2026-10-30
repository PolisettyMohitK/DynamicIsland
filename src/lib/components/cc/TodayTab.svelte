<script lang="ts">
  import { clock } from '$lib/clock.svelte';
  import { focus, LABEL } from '$lib/focus.svelte';
  import { mmss } from '$lib/store';
  import { todos } from '$lib/todos.svelte';
  import { water } from '$lib/water.svelte';
  import Digits from '../Digits.svelte';
  import Gauge from '../Gauge.svelte';
  import Icon from '../Icon.svelte';

  const SHOWN = 4;

  const date = $derived(new Date(clock.now));
  // Open tasks first, the soonest due on top; untimed ones after.
  const open = $derived(
    todos.tasks
      .filter((t) => t.doneOn === null)
      .toSorted((a, b) => (a.due ?? '99:99').localeCompare(b.due ?? '99:99')),
  );
</script>

<div class="grid">
  <div class="card date">
    <span class="weekday">{date.toLocaleDateString('en-GB', { weekday: 'long' })}</span>
    <b>{date.getDate()}</b>
    <span class="month">{date.toLocaleDateString('en-GB', { month: 'long' })}</span>
  </div>

  <div class="card tasks">
    <div class="line">
      <span class="head">To-do</span>
      <span class="head"><Digits value={`${todos.done}/${todos.tasks.length}`} /> done</span>
    </div>
    {#each open.slice(0, SHOWN) as t (t.id)}
      <button class="task" onclick={() => todos.toggle(t.id)}>
        <span class="box"></span>
        <span class="text">{t.text}</span>
        {#if t.due}<span class="due">{t.due}</span>{/if}
      </button>
    {:else}
      <p class="none">{todos.tasks.length ? 'All done today' : 'Nothing to do'}</p>
    {/each}
    {#if open.length > SHOWN}<span class="head">{open.length - SHOWN} more</span>{/if}
  </div>

  <button class="card small" style="--accent: var(--accent-focus)" onclick={() => focus.toggle()}>
    <span class="dot"><Icon name={focus.running ? 'pause' : 'play'} size={16} filled /></span>
    <span class="text">
      <b><Digits value={mmss(focus.remaining(clock.now))} /></b>
      <small>{LABEL[focus.kind]}{focus.running ? '' : ' · paused'}</small>
    </span>
  </button>

  <button class="card small" style="--accent: var(--accent-water)" onclick={() => water.drink()}>
    <Gauge value={water.percent / 100} size={32} stroke={4} accent="var(--accent-water)">
      <Icon name="drop" size={14} />
    </Gauge>
    <span class="text">
      <b><Digits value={`${water.ml}`} /> ml</b>
      <small>of {water.goalMl} · tap for {water.cupMl}</small>
    </span>
  </button>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: 150px 1fr 1fr;
    grid-template-rows: auto auto;
    gap: 8px;
  }
  .card {
    border: none;
    border-radius: 18px;
    background: var(--pill);
    color: inherit;
    font: inherit;
    text-align: left;
  }
  .date {
    grid-row: span 2;
    padding: 14px;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  .weekday {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--accent-focus);
  }
  .date b {
    font-size: 48px;
    font-weight: 600;
    line-height: 1.05;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }
  .month {
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .tasks {
    grid-column: span 2;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .line {
    display: flex;
    justify-content: space-between;
    padding: 0 2px 4px;
  }
  .head {
    font-size: 11.5px;
    color: var(--text-dim);
    padding: 0 2px;
  }
  .task {
    height: 28px;
    padding: 0 4px;
    border: none;
    border-radius: 8px;
    background: none;
    color: inherit;
    font: inherit;
    font-size: 12.5px;
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .task:hover {
    background: var(--pill);
  }
  .box {
    flex: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1.5px solid var(--accent-todo);
  }
  .task .text {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .due {
    font-size: 11.5px;
    color: var(--accent-todo);
    font-variant-numeric: tabular-nums;
  }
  .none {
    margin: 0;
    padding: 6px 2px;
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .small {
    height: 52px;
    padding: 0 10px;
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    transition:
      background 160ms,
      transform 160ms var(--ease-out);
  }
  .small:hover {
    background: var(--pill-hover);
  }
  .small:active {
    transform: scale(0.96);
  }
  .dot {
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: #111;
  }
  .small .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    line-height: 1.25;
  }
  .small b {
    font-size: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .small small {
    font-size: 11.5px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
