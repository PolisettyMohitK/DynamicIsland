// Choco the pet, from the island's side: its settings, whether it's home asleep, and
// waking it. The dog itself lives in its own window (src/routes/pet, src-tauri/src/pet.rs).
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { load, save } from './store';

export type Distance = 'close' | 'normal' | 'far';
export type Energy = 'calm' | 'playful';
export type PetSettings = {
  enabled: boolean;
  distance: Distance;
  energy: Energy;
  sleepAfterMin: number;
  nightSleep: boolean;
  sounds: boolean;
};
/** What's going on around it, for the pet page. */
export type PetMood = { music: boolean; listening: boolean; calm: boolean };

class Pet {
  settings = $state<PetSettings>(
    load('pet', {
      enabled: true,
      distance: 'normal',
      energy: 'calm',
      sleepAfterMin: 2,
      nightSleep: true,
      sounds: false,
    } satisfies PetSettings),
  );
  /** Asleep in the island (rather than out on the desktop). */
  home = $state(true);

  #mood = '';
  #last: PetMood | null = null;

  start() {
    invoke('pet_configure', { config: this.settings });
    const off = listen<boolean>('pet-home', (e) => {
      this.home = e.payload;
      // Its window is new each time it comes out: catch it up.
      if (!e.payload && this.#last) emit('pet-mood', this.#last);
    });
    return () => off.then((f) => f());
  }

  set(patch: Partial<PetSettings>) {
    Object.assign(this.settings, patch);
    this.settings.sleepAfterMin = Math.min(10, Math.max(1, this.settings.sleepAfterMin));
    save('pet', this.settings);
    invoke('pet_configure', { config: this.settings });
  }

  /** hover | voice | nudge (a reminder: it runs right up to the cursor). */
  wake(reason: 'hover' | 'voice' | 'nudge') {
    if (this.settings.enabled) invoke('pet_wake', { reason });
  }

  context(calm: boolean, night: boolean) {
    invoke('pet_context', { calm, night });
  }

  /** Sent only when it changes: each one resets how settled the dog is. */
  mood(m: PetMood) {
    const key = JSON.stringify(m);
    if (key === this.#mood) return;
    this.#mood = key;
    this.#last = m;
    emit('pet-mood', m);
  }
}

export const pet = new Pet();
