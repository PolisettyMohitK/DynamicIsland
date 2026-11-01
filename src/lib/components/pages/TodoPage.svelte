<script lang="ts">
  import { tick } from 'svelte';
  import { todos } from '$lib/todos.svelte';
  import { startTyping, stopTyping } from '$lib/ui.svelte';
  import Digits from '../Digits.svelte';
  import Icon from '../Icon.svelte';
  import IconButton from '../IconButton.svelte';
  import Pill from '../Pill.svelte';

  const ACCENT = 'var(--accent-todo)';

  let adding = $state(false);
  let editing = $state(false);
  let text = $state('');
  let due = $state('');
  let field: HTMLInputElement | undefined = $state();

  async function openForm() {
    editing = false;
    adding = true;
    await tick();
    if (field) startTyping(field);
  }

  function closeForm() {
    adding = false;
    text = '';
    due = '';
    stopTyping();
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    todos.add(text, due || null);
    text = '';
    due = '';
    field?.focus();
  }

  // Clicking outside the island (the window losing focus) closes the form.
  // Buttons inside the island take focus themselves and handle it.
  function onFocusOut(e: FocusEvent) {
    if (!e.relatedTarget) closeForm();
  }

  $effect(() => () => stopTyping());
</script>

<div class="summary">
  <b><Digits value={`${todos.done}/${todos.tasks.length}`} /></b>
  <span>{todos.tasks.length && todos.done === todos.tasks.length ? 'All done today' : 'Done today'}</span>
  <div class="buttons">
    <Pill primary accent={ACCENT} onclick={() => (adding ? closeForm() : openForm())}>
      <Icon name={adding ? 'x' : 'plus'} size={14} />{adding ? 'Cancel' : 'Task'}
    </Pill>
    <IconButton
      icon={editing ? 'check' : 'edit'}
      size={28}
      active={editing}
      quiet={!editing}
      accent={ACCENT}
      label={editing ? 'Done editing' : 'Edit tasks'}
      onclick={() => ((editing = !editing), adding && closeForm())}
    />
  </div>
</div>

<div class="card">
  {#if adding}
    <form onsubmit={submit} onfocusout={onFocusOut}>
      <input
        bind:this={field}
        bind:value={text}
        placeholder="New task"
        maxlength="80"
        onkeydown={(e) => e.key === 'Escape' && closeForm()}
      />
      <div class="row">
        <input type="time" bind:value={due} aria-label="Due time (optional)" />
        <button type="submit" class="add" aria-label="Add" disabled={!text.trim()}><Icon name="check" size={15} /></button>
      </div>
    </form>
  {/if}
  {#if todos.tasks.length}
    <ul>
      {#each todos.tasks as t (t.id)}
        <li class:done={t.doneOn}>
          <button class="task" onclick={() => todos.toggle(t.id)}>
            <span class="tick">
              <svg viewBox="0 0 12 12"><path d="M3 6.2l2 2 4-4.4" pathLength="1" /></svg>
            </span>
            <span class="text">{t.text}</span>
            {#if t.due}<span class="due">{t.due}</span>{/if}
          </button>
          {#if editing}
            <button class="remove" aria-label="Delete task" onclick={() => todos.remove(t.id)}>
              <Icon name="x" size={12} />
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {:else if !adding}
    <p class="empty"><Icon name="check" size={18} />Nothing to do</p>
  {/if}
</div>

<style>
  .summary {
    position: absolute;
    left: var(--edge);
    top: 40px;
    display: flex;
    flex-direction: column;
  }
  .summary b {
    font-family: var(--font-display);
    font-size: 48px;
    line-height: 56px;
    font-weight: 600;
    letter-spacing: -1px;
  }
  .summary span {
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .buttons {
    margin-top: 14px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .card {
    position: absolute;
    left: 236px;
    right: var(--edge);
    top: 44px;
    height: 108px;
    padding: 4px;
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.045);
    box-shadow: inset 0 0 0 1px var(--hairline);
    overflow-y: auto;
    scrollbar-width: none;
    /* Fade the list out at the bottom when it scrolls. */
    mask: linear-gradient(#000 82%, transparent);
  }
  form {
    display: flex;
    gap: 4px;
    margin-bottom: 2px;
    animation: open 240ms var(--ease-out);
  }
  @keyframes open {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .row {
    display: flex;
    gap: 4px;
  }
  input {
    min-width: 0;
    height: 28px;
    padding: 0 10px;
    border: none;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.28);
    box-shadow: inset 0 0 0 1px var(--hairline);
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
    outline: none;
    color-scheme: dark;
    transition: box-shadow 160ms;
  }
  input:focus {
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent-todo) 70%, transparent);
  }
  form > input {
    flex: 1;
  }
  input[type='time'] {
    width: 76px;
    padding: 0 6px;
  }
  .add {
    width: 28px;
    height: 28px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: var(--accent-todo);
    display: grid;
    place-items: center;
    color: #fff;
    cursor: pointer;
    transition:
      opacity 160ms,
      transform 240ms var(--spring);
  }
  .add:active {
    transform: scale(0.88);
  }
  .add:disabled {
    opacity: 0.35;
    cursor: default;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    animation: open 240ms var(--ease-out);
  }
  .task {
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 8px;
    border: none;
    border-radius: 12px;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
    display: flex;
    align-items: center;
    gap: 9px;
    cursor: pointer;
    text-align: left;
    transition: background 160ms;
  }
  .task:hover {
    background: rgba(255, 255, 255, 0.06);
  }
  .tick {
    width: 15px;
    height: 15px;
    flex: none;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1.5px var(--text-faint);
    display: grid;
    place-items: center;
    transition:
      background 220ms,
      box-shadow 220ms,
      transform 300ms var(--spring);
  }
  .task:active .tick {
    transform: scale(0.8);
  }
  .done .tick {
    background: var(--accent-todo);
    box-shadow: inset 0 0 0 1.5px var(--accent-todo);
  }
  .tick svg {
    width: 11px;
    height: 11px;
    fill: none;
    stroke: #fff;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
    transition: stroke-dashoffset 260ms var(--ease-out) 60ms;
  }
  .done .tick svg {
    stroke-dashoffset: 0;
  }
  .text {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: color 220ms;
  }
  .done .text {
    color: var(--text-faint);
    text-decoration: line-through;
  }
  .due {
    font-size: 11px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .remove {
    width: 22px;
    height: 22px;
    margin-right: 3px;
    flex: none;
    border: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--pill);
    color: var(--text);
    cursor: pointer;
    animation: pop 260ms var(--spring);
    transition: background 160ms;
  }
  @keyframes pop {
    from {
      transform: scale(0.4);
      opacity: 0;
    }
  }
  .remove:hover {
    background: #ff5a5f;
  }
  .empty {
    margin: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-faint);
  }
  /* On a side the list goes under the count, the card's full width. */
  :global(.side) .summary {
    top: 72px;
  }
  :global(.side) .card {
    left: var(--edge);
    top: 200px;
    height: 120px;
  }
</style>
