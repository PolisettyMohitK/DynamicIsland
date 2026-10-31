// The dashboard window changes settings through the island, which owns them: the dashboard
// sends a change, the island applies it and sends every setting back. Types only from the
// stores here, so the dashboard never runs the island's timers and reminders.
import type { ChocoSettings } from './choco.svelte';
import type { Kind } from './focus.svelte';
import type { PetSettings } from './pet.svelte';
import type { Colour } from './ui.svelte';

export type WaterStep = 'goalMl' | 'cupMl' | 'everyMin';

export type SettingsSnapshot = {
  choco: ChocoSettings;
  pet: PetSettings;
  /** Session lengths in minutes. */
  focusMin: Record<Kind, number>;
  water: { goalMl: number; cupMl: number; reminder: boolean; everyMin: number };
  colour: Colour;
  lyricsOn: boolean;
};

export type SettingsChange =
  | { choco: Partial<ChocoSettings> }
  | { pet: Partial<PetSettings> }
  | { focusMin: [Kind, number] }
  | { water: [WaterStep, number] }
  | { waterReminder: boolean }
  | { colour: Colour }
  | { lyricsOn: boolean };

/** Island → dashboard: every setting, after each change. */
export const SETTINGS = 'settings';
/** Dashboard → island: one change. */
export const SETTINGS_CHANGE = 'settings-change';
/** Dashboard → island: send the settings now (the dashboard just loaded). */
export const SETTINGS_GET = 'settings-get';
