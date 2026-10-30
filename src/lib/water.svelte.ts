// Daily water intake with a repeating reminder.
import { chime, popup } from './alerts';
import { load, save, today } from './store';

type Saved = {
  day: string;
  ml: number;
  goalMl: number;
  cupMl: number;
  everyMin: number;
  reminder: boolean;
  nextAt: number;
};

const DEFAULTS: Saved = {
  day: '',
  ml: 0,
  goalMl: 1500,
  cupMl: 250,
  everyMin: 60,
  reminder: true,
  nextAt: 0,
};

class Water {
  #s = $state<Saved>({ ...DEFAULTS, ...load<Partial<Saved>>('water', {}) });

  constructor() {
    this.#rollover();
    if (this.#s.nextAt < Date.now()) this.#s.nextAt = Date.now() + this.#s.everyMin * 60_000;
  }

  get ml() {
    return this.#s.ml;
  }
  get goalMl() {
    return this.#s.goalMl;
  }
  get cupMl() {
    return this.#s.cupMl;
  }
  get everyMin() {
    return this.#s.everyMin;
  }
  get reminder() {
    return this.#s.reminder;
  }
  get nextAt() {
    return this.#s.nextAt;
  }
  get percent() {
    return Math.min(100, Math.round((this.#s.ml / this.#s.goalMl) * 100));
  }

  drink() {
    this.#rollover();
    this.#s.ml += this.#s.cupMl;
    this.#s.nextAt = Date.now() + this.#s.everyMin * 60_000;
    this.#save();
  }

  undo() {
    this.#s.ml = Math.max(0, this.#s.ml - this.#s.cupMl);
    this.#save();
  }

  toggleReminder() {
    this.#s.reminder = !this.#s.reminder;
    this.#s.nextAt = Date.now() + this.#s.everyMin * 60_000;
    this.#save();
  }

  /** Nudge a setting by `steps` within its range. */
  adjust(key: 'goalMl' | 'cupMl' | 'everyMin', steps: number) {
    const [step, min, max] = { goalMl: [250, 500, 5000], cupMl: [50, 50, 1000], everyMin: [15, 15, 240] }[key];
    this.#s[key] = Math.min(max, Math.max(min, this.#s[key] + step * steps));
    if (key === 'everyMin') this.#s.nextAt = Date.now() + this.#s.everyMin * 60_000;
    this.#save();
  }

  /** Called every so often: remind if it's time and the goal isn't met yet. */
  check() {
    this.#rollover();
    const s = this.#s;
    if (!s.reminder || Date.now() < s.nextAt) return;
    s.nextAt = Date.now() + s.everyMin * 60_000;
    this.#save();
    if (s.ml >= s.goalMl) return;
    chime();
    popup({
      icon: 'drop',
      label: 'Time to drink water',
      value: `${(s.ml / 1000).toFixed(1)} / ${(s.goalMl / 1000).toFixed(1)} L`,
      accent: 'var(--accent-water)',
      hold: 4000,
    });
  }

  #rollover() {
    if (this.#s.day === today()) return;
    this.#s.day = today();
    this.#s.ml = 0;
    this.#save();
  }

  #save() {
    save('water', this.#s);
  }
}

export const water = new Water();
