<script lang="ts">
  // Everything you can change, in one quiet sheet inside the island.
  import { invoke } from '@tauri-apps/api/core';
  import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
  import { aiStatus, providerName, setKey, type AiStatus } from '$lib/ai';
  import { choco, type Sensitivity } from '$lib/choco.svelte';
  import { focus, LENGTH } from '$lib/focus.svelte';
  import { pet, type Distance, type Energy } from '$lib/pet.svelte';
  import { setColour, startTyping, stopTyping, ui, type Colour } from '$lib/ui.svelte';
  import { water } from '$lib/water.svelte';
  import IconButton from './IconButton.svelte';
  import Segmented from './Segmented.svelte';
  import Stepper from './Stepper.svelte';
  import Toggle from './Toggle.svelte';

  let { lyricsOn, onLyrics, onclose }: { lyricsOn: boolean; onLyrics: () => void; onclose: () => void } = $props();

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
  const minutes = (ms: number) => Math.round(ms / 60_000);

  let mics = $state<string[]>([]);
  let ai = $state<AiStatus | null>(null);
  const hasKey = $derived(!!ai?.services[ai.provider].key);
  let editingKey = $state(false);
  let keyText = $state('');
  let keyField = $state<HTMLInputElement>();
  let autostart = $state(false);

  $effect(() => {
    invoke<string[]>('choco_mics').then((m) => (mics = m));
    aiStatus().then((s) => (ai = s));
    isEnabled().then((on) => (autostart = on)).catch(() => {});
  });

  /** Default, then each microphone in turn. */
  function nextMic() {
    const all = [null, ...mics];
    const i = all.indexOf(choco.settings.mic);
    choco.set({ mic: all[(i + 1) % all.length] });
  }

  async function editKey() {
    editingKey = true;
    keyText = '';
    await Promise.resolve();
    if (keyField) startTyping(keyField);
  }

  async function saveKey() {
    const k = keyText.trim();
    editingKey = false;
    stopTyping();
    if (!k || !ai) return;
    await setKey(ai.provider, k);
    ai = await aiStatus();
  }

  async function setAutostart(on: boolean) {
    autostart = on;
    await (on ? enable() : disable()).catch(() => (autostart = !on));
  }
</script>

<div class="settings">
  <header>
    <span class="title">Settings</span>
    <IconButton icon="x" size={26} quiet label="Close settings" onclick={onclose} />
  </header>

  <div class="cols">
    <section>
      <h3>Choco</h3>
      <div class="row"><span>Hey Choco</span><Toggle label="Hey Choco" on={choco.settings.enabled} onchange={(on) => choco.set({ enabled: on })} /></div>
      <div class="row">
        <span>Hears you</span>
        <Segmented options={SENSITIVITY} value={choco.settings.sensitivity} onchange={(v) => choco.set({ sensitivity: v })} />
      </div>
      <div class="row">
        <span>Microphone</span>
        <button class="pick" title={choco.settings.mic ?? 'Windows default'} onclick={nextMic}>{choco.settings.mic ?? 'Default'}</button>
      </div>
      <div class="row">
        <span>Follow-ups</span>
        <Toggle label="Listen for a follow-up" on={choco.settings.followUps} onchange={(on) => choco.set({ followUps: on })} />
      </div>
      <div class="row">
        <span title="Pick the service and models on the dashboard">{ai ? providerName(ai.provider) : 'AI'} key</span>
        {#if editingKey}
          <input
            bind:this={keyField}
            bind:value={keyText}
            type="password"
            placeholder="Paste, then Enter"
            onkeydown={(e) => {
              if (e.key === 'Enter') saveKey();
              if (e.key === 'Escape') {
                editingKey = false;
                stopTyping();
              }
            }}
            onblur={() => editingKey && saveKey()}
          />
        {:else}
          <button class="pick" onclick={editKey}>{hasKey ? 'Saved · Change' : 'Add key'}</button>
        {/if}
      </div>

      <h3>Look</h3>
      <div class="row"><span>Colour</span><Segmented options={COLOURS} value={ui.colour} onchange={setColour} /></div>
      <div class="row"><span>Lyrics while playing</span><Toggle label="Lyrics in the live bar" on={lyricsOn} onchange={onLyrics} /></div>
    </section>

    <section>
      <h3>Focus</h3>
      <div class="row">
        <span>Focus</span>
        <Stepper label="focus length" value="{minutes(LENGTH.focus)} min" onstep={(s) => focus.setLength('focus', minutes(LENGTH.focus) + s * 5)} />
      </div>
      <div class="row">
        <span>Break</span>
        <Stepper label="break length" value="{minutes(LENGTH.break)} min" onstep={(s) => focus.setLength('break', minutes(LENGTH.break) + s)} />
      </div>

      <h3>Water</h3>
      <div class="row"><span>Daily goal</span><Stepper label="goal" value="{(water.goalMl / 1000).toFixed(water.goalMl % 1000 ? 2 : 1)} L" onstep={(s) => water.adjust('goalMl', s)} /></div>
      <div class="row"><span>Cup</span><Stepper label="cup size" value="{water.cupMl} ml" onstep={(s) => water.adjust('cupMl', s)} /></div>
      <div class="row"><span>Reminders</span><Toggle label="Water reminders" on={water.reminder} onchange={() => water.toggleReminder()} /></div>
      {#if water.reminder}
        <div class="row"><span>Every</span><Stepper label="reminder gap" value="{water.everyMin} min" onstep={(s) => water.adjust('everyMin', s)} /></div>
      {/if}

      <h3>System</h3>
      <div class="row"><span>Start with Windows</span><Toggle label="Start with Windows" on={autostart} onchange={setAutostart} /></div>
    </section>

    <section>
      <h3>Pet</h3>
      <div class="row"><span>Out on the desktop</span><Toggle label="Pet on the desktop" on={pet.settings.enabled} onchange={(on) => pet.set({ enabled: on })} /></div>
      {#if pet.settings.enabled}
        <div class="row"><span>Follows</span><Segmented options={DISTANCES} value={pet.settings.distance} onchange={(v) => pet.set({ distance: v })} /></div>
        <div class="row"><span>Energy</span><Segmented options={ENERGIES} value={pet.settings.energy} onchange={(v) => pet.set({ energy: v })} /></div>
        <div class="row">
          <span>Sleeps after</span>
          <Stepper label="sleep after" value="{pet.settings.sleepAfterMin} min" onstep={(st) => pet.set({ sleepAfterMin: pet.settings.sleepAfterMin + st })} />
        </div>
        <div class="row"><span>Sleeps at night</span><Toggle label="Sleep at night" on={pet.settings.nightSleep} onchange={(on) => pet.set({ nightSleep: on })} /></div>
        <div class="row"><span>Sounds</span><Toggle label="Pet sounds" on={pet.settings.sounds} onchange={(on) => pet.set({ sounds: on })} /></div>
      {/if}
    </section>
  </div>
</div>

<style>
  .settings {
    width: 720px;
    padding: 12px 22px 20px 28px;
  }
  header {
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .title {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-dim);
  }
  .cols {
    margin-top: 6px;
    display: grid;
    grid-template-columns: 1.15fr 1fr 1fr;
    gap: 28px;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  h3 {
    margin: 6px 0 2px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.4px;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  h3:not(:first-child) {
    margin-top: 12px;
  }
  .row {
    min-height: 28px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12.5px;
  }
  .row > span:first-child {
    color: var(--text-dim);
    white-space: nowrap;
  }
  .row :global(.seg) {
    height: 24px;
  }
  .row :global(.seg button) {
    padding: 0 8px;
    font-size: 11.5px;
  }
  .pick,
  input {
    max-width: 130px;
    height: 24px;
    padding: 0 10px;
    border: none;
    border-radius: 12px;
    background: var(--pill);
    color: var(--text);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
    transition: background 160ms;
  }
  .pick:hover {
    background: var(--pill-hover);
  }
  input {
    width: 130px;
    cursor: text;
    font-weight: 500;
    outline: none;
    box-shadow: inset 0 0 0 1px var(--hairline);
  }
  input::placeholder {
    color: var(--text-faint);
  }
</style>
