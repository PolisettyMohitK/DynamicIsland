// Pop-ups and the chime. Anything can raise a pop-up; Island.svelte shows it.
import type { IconName } from './icons';

export type Popup = {
  icon: IconName;
  label: string;
  value?: string;
  /** 0–100: draws a level bar (volume, brightness). */
  level?: number;
  tone?: 'ok' | 'bad';
  /** Icon colour. */
  accent?: string;
  /** How long it stays up, in ms. */
  hold?: number;
};

let show: ((p: Popup) => void) | null = null;

export function onPopup(handler: (p: Popup) => void) {
  show = handler;
  return () => {
    if (show === handler) show = null;
  };
}

export function popup(p: Popup) {
  show?.(p);
}

let audio: AudioContext | null = null;

/** Two soft notes, made on the fly so there's no sound file to ship. */
export function chime() {
  try {
    audio ??= new AudioContext();
    const t = audio.currentTime;
    [880, 1318.5].forEach((freq, i) => {
      const osc = audio!.createOscillator();
      const gain = audio!.createGain();
      osc.type = 'sine';
      osc.frequency.value = freq;
      const start = t + i * 0.18;
      gain.gain.setValueAtTime(0, start);
      gain.gain.linearRampToValueAtTime(0.18, start + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.9);
      osc.connect(gain).connect(audio!.destination);
      osc.start(start);
      osc.stop(start + 1);
    });
  } catch {
    // no audio device; the pop-up still shows
  }
}
