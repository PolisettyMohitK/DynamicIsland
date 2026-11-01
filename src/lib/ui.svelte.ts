// Island-wide UI state that pages need to change.
import { invoke } from '@tauri-apps/api/core';
import { load, save } from './store';

/** Soft: a muted colour per page. Mono: one quiet colour for everything. */
export type Colour = 'soft' | 'mono';

/** How the music waves behind Now Playing are drawn. */
export type WaveStyle = 'line' | 'bars' | 'mirror' | 'dots';

/** Where a Claude Code session opens from the island. */
export type ClaudeOpen = 'desktop' | 'terminal';

export const ui = $state({
  /** A text field has the keyboard; the island stays open until it's done. */
  typing: false,
  colour: load<Colour>('colour', 'soft'),
  /** Lyrics in the live bar and beside Now Playing. */
  lyricsOn: load('lyricsOn', false),
  /** Docked on a side, the resting pill tucks into the edge with just a sliver showing. */
  tuck: load('tuck', false),
  wave: load<WaveStyle>('wave', 'line'),
  claudeOpen: load<ClaudeOpen>('claudeOpen', 'desktop'),
});

export function setClaudeOpen(where: ClaudeOpen) {
  ui.claudeOpen = where;
  save('claudeOpen', where);
}

export function setWave(style: WaveStyle) {
  ui.wave = style;
  save('wave', style);
}

export function setTuck(on: boolean) {
  ui.tuck = on;
  save('tuck', on);
}

export function setColour(c: Colour) {
  ui.colour = c;
  save('colour', c);
  document.documentElement.dataset.colour = c;
}

export function setLyrics(on: boolean) {
  ui.lyricsOn = on;
  save('lyricsOn', on);
}

/** The island window normally never takes focus. Let it while a field is in use. */
export async function startTyping(field: HTMLElement) {
  ui.typing = true;
  await invoke('set_typing', { on: true }).catch(() => {});
  field.focus();
}

export function stopTyping() {
  if (!ui.typing) return;
  ui.typing = false;
  invoke('set_typing', { on: false }).catch(() => {});
}
