// Island-wide UI state that pages need to change.
import { invoke } from '@tauri-apps/api/core';
import { load, save } from './store';

/** Soft: a muted colour per page. Mono: one quiet colour for everything. */
export type Colour = 'soft' | 'mono';

export const ui = $state({
  /** A text field has the keyboard; the island stays open until it's done. */
  typing: false,
  colour: load<Colour>('colour', 'soft'),
});

export function setColour(c: Colour) {
  ui.colour = c;
  save('colour', c);
  document.documentElement.dataset.colour = c;
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
