// Pomodoro timer. Counts against an end timestamp, so it stays right while
// the window is hidden or the app restarts.
import { chime, popup } from './alerts';
import { load, save } from './store';

export type Kind = 'focus' | 'break';

/** Session lengths in ms; changed in Settings. */
export const LENGTH = $state<Record<Kind, number>>(load('focusLengths', { focus: 25 * 60_000, break: 5 * 60_000 }));
export const LABEL: Record<Kind, string> = { focus: 'Focus time', break: 'Short break' };

type Saved = { kind: Kind; endAt: number | null; left: number };

class Focus {
  kind = $state<Kind>('focus');
  /** Set while running. */
  endAt = $state<number | null>(null);
  /** Time left while paused. */
  left = $state(LENGTH.focus);

  #timer: ReturnType<typeof setTimeout> | undefined;

  constructor() {
    const s = load<Saved | null>('focus', null);
    if (!s) return;
    this.kind = s.kind;
    this.left = s.left;
    if (s.endAt !== null && s.endAt > Date.now()) {
      this.endAt = s.endAt;
      this.#schedule();
    } else if (s.endAt !== null) {
      // Ran out while the app was closed: move on to the next session quietly.
      this.#next();
    }
  }

  get running() {
    return this.endAt !== null;
  }

  remaining(now: number) {
    return this.endAt === null ? this.left : Math.max(0, this.endAt - now);
  }

  toggle() {
    if (this.running) this.pause();
    else this.start();
  }

  start() {
    if (this.running) return;
    this.endAt = Date.now() + this.left;
    this.#schedule();
    this.#save();
  }

  pause() {
    if (this.endAt === null) return;
    this.left = Math.max(0, this.endAt - Date.now());
    this.endAt = null;
    clearTimeout(this.#timer);
    this.#save();
  }

  reset() {
    clearTimeout(this.#timer);
    this.endAt = null;
    this.left = LENGTH[this.kind];
    this.#save();
  }

  /** Settings: new length in minutes. A paused session of that kind starts over at the new length. */
  setLength(kind: Kind, minutes: number) {
    LENGTH[kind] = minutes * 60_000;
    save('focusLengths', LENGTH);
    if (kind === this.kind && !this.running) this.left = LENGTH[kind];
    this.#save();
  }

  /** Switch between focus and break; starts fresh. */
  choose(kind: Kind) {
    this.kind = kind;
    this.reset();
  }

  #schedule() {
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => this.#finish(), this.endAt! - Date.now());
  }

  #finish() {
    const done = this.kind;
    this.#next();
    chime();
    popup({
      icon: done === 'focus' ? 'coffee' : 'timer',
      label: done === 'focus' ? 'Focus done' : 'Break over',
      value: done === 'focus' ? 'Take a break' : 'Back to it',
      accent: 'var(--accent-focus)',
      hold: 4000,
    });
  }

  #next() {
    this.kind = this.kind === 'focus' ? 'break' : 'focus';
    this.endAt = null;
    this.left = LENGTH[this.kind];
    this.#save();
  }

  #save() {
    save('focus', { kind: this.kind, endAt: this.endAt, left: this.left } satisfies Saved);
  }
}

export const focus = new Focus();
