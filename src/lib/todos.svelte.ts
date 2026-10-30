// To-do list with optional due times (today, "HH:MM"). A due task raises a pop-up once.
import { chime, popup } from './alerts';
import { load, save, today } from './store';

export type Task = {
  id: number;
  text: string;
  due: string | null;
  /** Day the task was ticked off, or null while open. */
  doneOn: string | null;
  /** Day its due pop-up was shown. */
  alertedOn: string | null;
};

class Todos {
  // Ticked-off tasks only count for the day they were done.
  tasks = $state<Task[]>(load<Task[]>('todos', []).filter((t) => t.doneOn === null || t.doneOn === today()));

  get done() {
    return this.tasks.filter((t) => t.doneOn !== null).length;
  }

  add(text: string, due: string | null) {
    text = text.trim();
    if (!text) return;
    this.tasks.push({ id: Date.now(), text, due: due || null, doneOn: null, alertedOn: null });
    this.#save();
  }

  toggle(id: number) {
    const t = this.tasks.find((t) => t.id === id);
    if (!t) return;
    t.doneOn = t.doneOn ? null : today();
    this.#save();
  }

  remove(id: number) {
    this.tasks = this.tasks.filter((t) => t.id !== id);
    this.#save();
  }

  /** Called every so often: pop up any open task whose time has come. */
  check() {
    const day = today();
    const now = new Date().toTimeString().slice(0, 5);
    // Drop tasks finished on earlier days once the date rolls over.
    if (this.tasks.some((t) => t.doneOn !== null && t.doneOn !== day)) {
      this.tasks = this.tasks.filter((t) => t.doneOn === null || t.doneOn === day);
    }
    for (const t of this.tasks) {
      if (t.doneOn || !t.due || t.due > now || t.alertedOn === day) continue;
      t.alertedOn = day;
      chime();
      popup({ icon: 'bell', label: t.text, value: t.due, accent: 'var(--accent-todo)', hold: 4000 });
    }
    this.#save();
  }

  #save() {
    save('todos', this.tasks);
  }
}

export const todos = new Todos();
