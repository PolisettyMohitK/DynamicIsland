// Control Centre state: Windows switches read when it opens, flipped optimistically.
import { invoke } from '@tauri-apps/api/core';

export type Output = { id: string; name: string; default: boolean };
export type ControlState = {
  wifi: boolean | null;
  bluetooth: boolean | null;
  dark: boolean;
  brightness: number | null;
  outputs: Output[];
};
export type PowerAction = 'lock' | 'sleep' | 'signout' | 'restart' | 'shutdown';

class Control {
  state = $state<ControlState | null>(null);
  error = $state('');
  #brightness: { sending: boolean; next: number | null } = { sending: false, next: null };

  async refresh() {
    try {
      this.state = await invoke<ControlState>('control_state');
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Show the change at once; if Windows refuses, read back what's really true. */
  async #flip(apply: (s: ControlState) => void, call: Promise<unknown>) {
    if (this.state) apply(this.state);
    try {
      await call;
      this.error = '';
    } catch (e) {
      this.error = String(e);
      await this.refresh();
    }
  }

  setRadio(kind: 'wifi' | 'bluetooth', on: boolean) {
    return this.#flip((s) => (s[kind] = on), invoke('control_set_radio', { kind, on }));
  }

  setDark(on: boolean) {
    return this.#flip((s) => (s.dark = on), invoke('control_set_dark', { on }));
  }

  setOutput(id: string) {
    return this.#flip(
      (s) => s.outputs.forEach((o) => (o.default = o.id === id)),
      invoke('control_set_output', { id }),
    );
  }

  /** Dragging sends a stream of levels; WMI is slow, so only the latest waiting one goes next. */
  async setBrightness(level: number) {
    if (this.state) this.state.brightness = level;
    const b = this.#brightness;
    b.next = level;
    if (b.sending) return;
    b.sending = true;
    while (b.next !== null) {
      const send = b.next;
      b.next = null;
      try {
        await invoke('control_set_brightness', { level: send });
      } catch (e) {
        this.error = String(e);
      }
    }
    b.sending = false;
  }

  power(action: PowerAction) {
    return invoke('control_power', { action }).catch((e) => (this.error = String(e)));
  }
}

export const control = new Control();
