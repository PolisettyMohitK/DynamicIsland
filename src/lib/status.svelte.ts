// System state from the backend, and the pop-ups its changes deserve.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { popup } from './alerts';

export type Status = {
  volume: number;
  muted: boolean;
  output: string;
  battery: { percent: number; charging: boolean } | null;
  network: { wifi: boolean; name: string } | null;
};

const LOW = [20, 10];
const HEADPHONES = /head|ear|bud|pod|airpods|bluetooth|hands-free/i;

class SystemStatus {
  current = $state<Status | null>(null);

  start() {
    const stop = [
      listen<Status>('status', (e) => this.#update(e.payload)),
      listen<number>('brightness', (e) =>
        popup({ icon: 'sun', label: 'Brightness', level: e.payload, value: String(e.payload) }),
      ),
    ];
    invoke<Status>('status_current').then((s) => (this.current ??= s));
    return () => stop.forEach((p) => p.then((off) => off()));
  }

  setVolume(level: number) {
    if (this.current) this.current.volume = Math.round(level);
    return invoke('set_volume', { level: level / 100 });
  }

  toggleMute() {
    return invoke('toggle_mute');
  }

  #update(next: Status) {
    const prev = this.current;
    this.current = next;
    if (!prev) return;

    // A new output device usually reports a new volume too; the device is the news.
    if (next.output && next.output !== prev.output) {
      popup({
        icon: HEADPHONES.test(next.output) ? 'headphones' : 'speaker',
        label: next.output,
        value: 'Connected',
        tone: 'ok',
        hold: 2000,
      });
    } else if (next.volume !== prev.volume || next.muted !== prev.muted) {
      const silent = next.muted || next.volume === 0;
      popup({
        icon: silent ? 'mute' : 'volume',
        label: 'Volume',
        level: next.muted ? 0 : next.volume,
        value: next.muted ? 'Muted' : String(next.volume),
      });
    }

    const a = prev.battery;
    const b = next.battery;
    if (a && b && a.charging !== b.charging) {
      popup({
        icon: b.charging ? 'bolt' : 'battery',
        label: b.charging ? 'Charging' : 'On battery',
        value: `${b.percent}%`,
        tone: b.charging ? 'ok' : undefined,
      });
    } else if (a && b && !b.charging && LOW.some((l) => a.percent > l && b.percent <= l)) {
      popup({ icon: 'battery', label: 'Low battery', value: `${b.percent}%`, tone: 'bad', hold: 3000 });
    }

    const n = prev.network;
    const m = next.network;
    if (n && !m) {
      popup({ icon: 'wifi-off', label: 'Disconnected', value: 'Offline', tone: 'bad', hold: 2000 });
    } else if (m && (!n || n.name !== m.name)) {
      popup({ icon: 'wifi', label: m.name || (m.wifi ? 'Wi-Fi' : 'Network'), value: 'Connected', tone: 'ok' });
    }
  }
}

export const status = new SystemStatus();
