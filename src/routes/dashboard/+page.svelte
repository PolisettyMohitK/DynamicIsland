<script lang="ts">
  // The dashboard: every setting in a plain window, opened from the Start menu or the tray.
  // Settings belong to the island (see settings-link.ts); this sends changes and shows what
  // comes back. It steps aside (minimizes) whenever the island is opened or Choco wakes.
  import '$lib/theme.css';
  import { invoke } from '@tauri-apps/api/core';
  import { emit, listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
  import { aiModels, aiStatus, PROVIDERS, providerName, setKey, setModel, setProvider, type AiStatus, type Job, type Provider } from '$lib/ai';
  import type { ChocoSettings, Sensitivity } from '$lib/choco.svelte';
  import type { Distance, Energy, PetSettings } from '$lib/pet.svelte';
  import {
    SETTINGS,
    SETTINGS_CHANGE,
    SETTINGS_GET,
    type SettingsChange,
    type SettingsSnapshot,
  } from '$lib/settings-link';
  import type { Colour } from '$lib/ui.svelte';
  import Segmented from '$lib/components/Segmented.svelte';
  import Stepper from '$lib/components/Stepper.svelte';
  import Toggle from '$lib/components/Toggle.svelte';

  const SENSITIVITY: { value: Sensitivity; label: string }[] = [
    { value: 'low', label: 'Low' },
    { value: 'normal', label: 'Normal' },
    { value: 'high', label: 'High' },
  ];
  const COLOURS: { value: Colour; label: string }[] = [
    { value: 'soft', label: 'Soft' },
    { value: 'mono', label: 'Mono' },
  ];
  const DISTANCES: { value: Distance; label: string }[] = [
    { value: 'close', label: 'Near' },
    { value: 'normal', label: 'Mid' },
    { value: 'far', label: 'Far' },
  ];
  const ENERGIES: { value: Energy; label: string }[] = [
    { value: 'calm', label: 'Calm' },
    { value: 'playful', label: 'Playful' },
  ];

  let s = $state<SettingsSnapshot | null>(null);
  let mics = $state<string[]>([]);
  let ai = $state<AiStatus | null>(null);
  let keyText = $state<Record<Provider, string>>({ opencode: '', openai: '', gemini: '' });
  let autostart = $state(false);
  /** What the model dropdowns offer, from the service in use; and which one is being typed into. */
  let offered = $state<Record<Job, string[]>>({ chat: [], computer: [] });
  let typing = $state<Record<Job, boolean>>({ chat: false, computer: false });
  const OTHER = '(other)';
  /** The lists only change with the service or its key, not every time a model is picked. */
  const listFor = $derived(ai ? `${ai.provider}:${ai.services[ai.provider].key}` : '');

  const change = (c: SettingsChange) => emit(SETTINGS_CHANGE, c);
  const setChoco = (patch: Partial<ChocoSettings>) => change({ choco: patch });
  const setPet = (patch: Partial<PetSettings>) => change({ pet: patch });
  const refreshAi = async () => (ai = await aiStatus());

  async function saveKey(p: Provider) {
    const key = keyText[p].trim();
    if (!key) return;
    await setKey(p, key);
    keyText[p] = '';
    await refreshAi();
  }

  async function removeKey(p: Provider) {
    await setKey(p, '');
    await refreshAi();
  }

  async function pickProvider(p: Provider) {
    await setProvider(p);
    await refreshAi();
  }

  async function pickModel(job: Job, model: string) {
    if (!ai) return;
    await setModel(ai.provider, job, model);
    await refreshAi();
  }

  /** The default, your pick and whatever the service lists, without repeats. */
  function choices(job: Job) {
    const svc = ai!.services[ai!.provider];
    return [...new Set([job === 'chat' ? svc.defaultChat : svc.defaultComputer, svc[job], ...offered[job]])];
  }

  const focusNow = (node: HTMLElement) => node.focus();

  /** Leaving the "Other…" box: a name was typed, use it. */
  function typed(job: Job, name: string) {
    typing[job] = false;
    if (name.trim()) pickModel(job, name);
  }

  async function setAutostart(on: boolean) {
    autostart = on;
    await (on ? enable() : disable()).catch(() => (autostart = !on));
  }

  /** Out of the way while the island is in use. Minimizing a hidden window would show it. */
  async function stepAside() {
    const win = getCurrentWindow();
    if ((await win.isVisible()) && !(await win.isMinimized())) await win.minimize();
  }

  $effect(() => {
    const [p, key] = listFor.split(':') as [Provider, string];
    offered = { chat: [], computer: [] };
    typing = { chat: false, computer: false };
    if (key !== 'true') return;
    let live = true;
    Promise.all([aiModels(p, 'chat'), aiModels(p, 'computer')]).then(([chat, computer]) => live && (offered = { chat, computer }));
    return () => (live = false);
  });

  $effect(() => {
    invoke<string[]>('choco_mics').then((m) => (mics = m));
    refreshAi();
    isEnabled().then((on) => (autostart = on)).catch(() => {});
    const offs = [
      listen<SettingsSnapshot>(SETTINGS, (e) => (s = e.payload)),
      listen<boolean>('island-hover', (e) => e.payload && stepAside()),
      listen<{ state: string; followUp?: boolean }>('choco', (e) => {
        if (e.payload.state === 'listening' && !e.payload.followUp) stepAside();
      }),
    ];
    emit(SETTINGS_GET);
    return () => offs.forEach((p) => p.then((off) => off()));
  });
</script>

{#snippet modelPicker(job: Job)}
  {@const svc = ai!.services[ai!.provider]}
  {@const def = job === 'chat' ? svc.defaultChat : svc.defaultComputer}
  {#if typing[job]}
    <input
      class="model"
      placeholder="Model name"
      use:focusNow
      onblur={(e) => typed(job, e.currentTarget.value)}
      onkeydown={(e) => {
        if (e.key === 'Enter') e.currentTarget.blur();
        if (e.key === 'Escape') {
          e.currentTarget.value = '';
          e.currentTarget.blur();
        }
      }}
    />
  {:else}
    <select
      class="model"
      value={svc[job]}
      onchange={(e) => (e.currentTarget.value === OTHER ? (typing[job] = true) : pickModel(job, e.currentTarget.value))}
    >
      {#each choices(job) as m}<option value={m}>{m === def ? `${m} (default)` : m}</option>{/each}
      <option value={OTHER}>Other…</option>
    </select>
  {/if}
{/snippet}

<main>
  <header>
    <h1>Dynamic Island</h1>
    <p>Hover the island at the top of your screen, or say “Hey Choco”. Closing this window keeps the island running.</p>
  </header>

  {#if !s}
    <p class="wait">Waiting for the island…</p>
  {:else}
    <div class="grid">
      <section>
        <h2>Choco</h2>
        <div class="row"><span>Hey Choco</span><Toggle label="Hey Choco" on={s.choco.enabled} onchange={(on) => setChoco({ enabled: on })} /></div>
        <div class="row">
          <span>Hears you</span>
          <Segmented options={SENSITIVITY} value={s.choco.sensitivity} onchange={(v) => setChoco({ sensitivity: v })} />
        </div>
        <div class="row">
          <span>Microphone</span>
          <select value={s.choco.mic ?? ''} onchange={(e) => setChoco({ mic: e.currentTarget.value || null })}>
            <option value="">Windows default</option>
            {#each mics as m}<option value={m}>{m}</option>{/each}
          </select>
        </div>
        <div class="row">
          <span>Follow-ups</span>
          <Toggle label="Listen for a follow-up" on={s.choco.followUps} onchange={(on) => setChoco({ followUps: on })} />
        </div>
      </section>

      {#if ai}
        {@const svc = ai.services[ai.provider]}
        <section class="wide">
          <h2>AI</h2>
          <p class="hint">Answers questions, uses your computer and remembers you. Plain commands work without a key.</p>
          <div class="row"><span>Use</span><Segmented options={PROVIDERS} value={ai.provider} onchange={pickProvider} /></div>
          {#each PROVIDERS as p}
            <div class="row">
              <span>{p.label} key</span>
              {#if ai.services[p.value].key}
                <span class="saved">Saved <button class="link" onclick={() => removeKey(p.value)}>Remove</button></span>
              {:else}
                <span class="key">
                  <input
                    type="password"
                    placeholder="Paste key"
                    bind:value={keyText[p.value]}
                    onkeydown={(e) => e.key === 'Enter' && saveKey(p.value)}
                  />
                  <button class="btn" disabled={!keyText[p.value].trim()} onclick={() => saveKey(p.value)}>Save</button>
                </span>
              {/if}
            </div>
          {/each}

          <h2>{providerName(ai.provider)} models</h2>
          <div class="row">
            <span>Questions</span>
            {@render modelPicker('chat')}
          </div>
          <div class="row">
            <span>Computer use</span>
            {@render modelPicker('computer')}
          </div>
          <p class="hint">
            Computer use needs a model that can see images and call tools. Pick “Other…” to type any model name.
            {#if !svc.key}<b>Add a {providerName(ai.provider)} key to use it and see all its models.</b>{/if}
          </p>
        </section>
      {/if}

      <section>
        <h2>Look</h2>
        <div class="row"><span>Colour</span><Segmented options={COLOURS} value={s.colour} onchange={(v) => change({ colour: v })} /></div>
        <div class="row"><span>Lyrics while playing</span><Toggle label="Lyrics" on={s.lyricsOn} onchange={(on) => change({ lyricsOn: on })} /></div>

        <h2>System</h2>
        <div class="row"><span>Start with Windows</span><Toggle label="Start with Windows" on={autostart} onchange={setAutostart} /></div>
      </section>

      <section>
        <h2>Focus</h2>
        <div class="row">
          <span>Focus</span>
          <Stepper label="focus length" value="{s.focusMin.focus} min" onstep={(st) => change({ focusMin: ['focus', s!.focusMin.focus + st * 5] })} />
        </div>
        <div class="row">
          <span>Break</span>
          <Stepper label="break length" value="{s.focusMin.break} min" onstep={(st) => change({ focusMin: ['break', s!.focusMin.break + st] })} />
        </div>

        <h2>Water</h2>
        <div class="row">
          <span>Daily goal</span>
          <Stepper label="goal" value="{(s.water.goalMl / 1000).toFixed(s.water.goalMl % 1000 ? 2 : 1)} L" onstep={(st) => change({ water: ['goalMl', st] })} />
        </div>
        <div class="row"><span>Cup</span><Stepper label="cup size" value="{s.water.cupMl} ml" onstep={(st) => change({ water: ['cupMl', st] })} /></div>
        <div class="row"><span>Reminders</span><Toggle label="Water reminders" on={s.water.reminder} onchange={(on) => change({ waterReminder: on })} /></div>
        {#if s.water.reminder}
          <div class="row"><span>Every</span><Stepper label="reminder gap" value="{s.water.everyMin} min" onstep={(st) => change({ water: ['everyMin', st] })} /></div>
        {/if}
      </section>

      <section>
        <h2>Pet</h2>
        <div class="row"><span>Out on the desktop</span><Toggle label="Pet on the desktop" on={s.pet.enabled} onchange={(on) => setPet({ enabled: on })} /></div>
        {#if s.pet.enabled}
          <div class="row"><span>Follows</span><Segmented options={DISTANCES} value={s.pet.distance} onchange={(v) => setPet({ distance: v })} /></div>
          <div class="row"><span>Energy</span><Segmented options={ENERGIES} value={s.pet.energy} onchange={(v) => setPet({ energy: v })} /></div>
          <div class="row">
            <span>Sleeps after</span>
            <Stepper label="sleep after" value="{s.pet.sleepAfterMin} min" onstep={(st) => setPet({ sleepAfterMin: s!.pet.sleepAfterMin + st })} />
          </div>
          <div class="row"><span>Sleeps at night</span><Toggle label="Sleep at night" on={s.pet.nightSleep} onchange={(on) => setPet({ nightSleep: on })} /></div>
          <div class="row"><span>Sounds</span><Toggle label="Pet sounds" on={s.pet.sounds} onchange={(on) => setPet({ sounds: on })} /></div>
        {/if}
      </section>
    </div>
  {/if}
</main>

<style>
  :global(html),
  :global(body) {
    height: 100%;
    background: #161618;
    overflow: auto;
    color-scheme: dark;
  }
  main {
    max-width: 860px;
    margin: 0 auto;
    padding: 28px 24px 32px;
  }
  header h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 600;
  }
  header p,
  .wait {
    margin: 6px 0 0;
    font-size: 13px;
    color: var(--text-dim);
  }
  .grid {
    margin-top: 22px;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
    gap: 16px;
  }
  section {
    padding: 14px 18px 16px;
    border-radius: 14px;
    background: #202023;
    box-shadow: inset 0 0 0 1px var(--hairline);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  section.wide {
    grid-column: 1 / -1;
  }
  .hint {
    margin: 0 0 4px;
    font-size: 12px;
    line-height: 1.45;
    color: var(--text-faint);
  }
  .hint b {
    font-weight: 600;
    color: var(--text-dim);
  }
  .model {
    width: 240px;
    user-select: text;
  }
  h2 {
    margin: 2px 0 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.4px;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  h2:not(:first-child) {
    margin-top: 14px;
  }
  .row {
    min-height: 32px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    font-size: 13px;
  }
  .row > span:first-child {
    color: var(--text-dim);
    white-space: nowrap;
  }
  select,
  input {
    width: 170px;
    height: 28px;
    padding: 0 10px;
    border: none;
    border-radius: 8px;
    background: var(--pill);
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
    outline: none;
    box-shadow: inset 0 0 0 1px var(--hairline);
  }
  select:focus,
  input:focus {
    box-shadow: inset 0 0 0 1px var(--text-faint);
  }
  option {
    background: #202023;
  }
  .key,
  .saved {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .key input {
    width: 130px;
    user-select: text;
  }
  .btn,
  .link {
    border: none;
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn {
    height: 28px;
    padding: 0 12px;
    border-radius: 8px;
    background: var(--pill-hover);
    color: var(--text);
  }
  .btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .link {
    padding: 0;
    background: none;
    color: var(--text-dim);
    text-decoration: underline;
  }
</style>
