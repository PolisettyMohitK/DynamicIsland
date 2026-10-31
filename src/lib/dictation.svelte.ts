// Hold Right Ctrl and talk: the backend records, transcribes and types; this mirrors it on the island.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { load, save } from './store';

type BackendEvent =
  | { state: 'listening' | 'thinking' | 'idle' }
  | { state: 'typed'; text: string }
  | { state: 'error'; message: string };

/** How long the typed text (or an error) stays on the island. */
const SHOW_MS = 1800;

class Dictation {
  state = $state<'idle' | 'listening' | 'thinking' | 'done'>('idle');
  level = $state(0);
  /** What was typed, or what went wrong. */
  text = $state('');
  failed = $state(false);
  enabled = $state(load('dictation', true));
  #timer: ReturnType<typeof setTimeout> | undefined;

  start() {
    invoke('dictation_set_enabled', { on: this.enabled });
    const stop = [
      listen<BackendEvent>('dictation', (e) => this.#on(e.payload)),
      listen<number>('dictation-level', (e) => (this.level = e.payload)),
    ];
    return () => stop.forEach((p) => p.then((f) => f()));
  }

  setEnabled(on: boolean) {
    this.enabled = on;
    save('dictation', on);
    invoke('dictation_set_enabled', { on });
  }

  #on(e: BackendEvent) {
    clearTimeout(this.#timer);
    if (e.state === 'listening' || e.state === 'thinking') {
      this.state = e.state;
      this.text = '';
      this.failed = false;
      return;
    }
    this.level = 0;
    if (e.state === 'idle') {
      this.state = 'idle';
      return;
    }
    this.state = 'done';
    this.failed = e.state === 'error';
    this.text = e.state === 'error' ? e.message : e.state === 'typed' ? e.text : '';
    this.#timer = setTimeout(() => (this.state = 'idle'), SHOW_MS);
  }
}

export const dictation = new Dictation();
