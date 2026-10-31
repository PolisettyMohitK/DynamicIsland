// The island's side of the dashboard link (see settings-link.ts): applies changes with the
// same calls the in-island Settings sheet uses, and sends the settings whenever they change.
import { emit, listen } from '@tauri-apps/api/event';
import { choco } from './choco.svelte';
import { focus, LENGTH } from './focus.svelte';
import { pet } from './pet.svelte';
import { SETTINGS, SETTINGS_CHANGE, SETTINGS_GET, type SettingsChange, type SettingsSnapshot } from './settings-link';
import { setColour, setLyrics, ui } from './ui.svelte';
import { water } from './water.svelte';

function snapshot(): SettingsSnapshot {
  return {
    choco: $state.snapshot(choco.settings),
    pet: $state.snapshot(pet.settings),
    focusMin: { focus: LENGTH.focus / 60_000, break: LENGTH.break / 60_000 },
    water: { goalMl: water.goalMl, cupMl: water.cupMl, reminder: water.reminder, everyMin: water.everyMin },
    colour: ui.colour,
    lyricsOn: ui.lyricsOn,
  };
}

function apply(c: SettingsChange) {
  if ('choco' in c) choco.set(c.choco);
  else if ('pet' in c) pet.set(c.pet);
  else if ('focusMin' in c) focus.setLength(...c.focusMin);
  else if ('water' in c) water.adjust(...c.water);
  else if ('waterReminder' in c) {
    if (water.reminder !== c.waterReminder) water.toggleReminder();
  } else if ('colour' in c) setColour(c.colour);
  else if ('lyricsOn' in c) setLyrics(c.lyricsOn);
}

/** Call from inside an effect; returns the cleanup. */
export function serveSettings() {
  $effect(() => {
    emit(SETTINGS, snapshot());
  });
  const offs = [
    listen<SettingsChange>(SETTINGS_CHANGE, (e) => apply(e.payload)),
    listen(SETTINGS_GET, () => emit(SETTINGS, snapshot())),
  ];
  return () => offs.forEach((p) => p.then((off) => off()));
}
