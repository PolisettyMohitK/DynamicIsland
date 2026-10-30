// Files parked on the island. Only paths are kept; the files stay where they are.
import { invoke } from '@tauri-apps/api/core';
import { load, save } from './store';

export type Item = { path: string; name: string; ext: string };

const MAX_ITEMS = 24;

function item(path: string): Item {
  const name = path.split(/[\\/]/).pop() || path;
  const dot = name.lastIndexOf('.');
  return { path, name, ext: dot > 0 ? name.slice(dot + 1).toLowerCase() : '' };
}

class Shelf {
  items = $state<Item[]>(load<string[]>('shelf', []).map(item));
  /** Image previews by path (object URLs). */
  thumbs = $state<Record<string, string>>({});
  /** Files are being dragged over the island. */
  hovering = $state(false);

  constructor() {
    // Forget files that were moved or deleted since last time.
    const paths = this.items.map((i) => i.path);
    if (paths.length) {
      invoke<boolean[]>('shelf_exists', { paths }).then((exists) => {
        this.items = this.items.filter((_, i) => exists[i] !== false);
        this.#save();
        this.items.forEach((i) => this.#thumb(i.path));
      });
    }
  }

  add(paths: string[]) {
    const fresh = paths.filter((p) => !this.items.some((i) => i.path === p));
    this.items = [...fresh.map(item), ...this.items].slice(0, MAX_ITEMS);
    fresh.forEach((p) => this.#thumb(p));
    this.#save();
  }

  remove(path: string) {
    this.items = this.items.filter((i) => i.path !== path);
    if (this.thumbs[path]) {
      URL.revokeObjectURL(this.thumbs[path]);
      delete this.thumbs[path];
    }
    this.#save();
  }

  clear() {
    [...this.items].forEach((i) => this.remove(i.path));
  }

  open(path: string) {
    invoke('shelf_open', { path }).catch(() => this.remove(path));
  }

  dragOut(path: string) {
    invoke('shelf_drag', { paths: [path] }).catch(() => this.remove(path));
  }

  async #thumb(path: string) {
    const buf = await invoke<ArrayBuffer>('shelf_thumb', { path }).catch(() => null);
    if (buf?.byteLength && this.items.some((i) => i.path === path)) {
      this.thumbs[path] = URL.createObjectURL(new Blob([buf]));
    }
  }

  #save() {
    save(
      'shelf',
      this.items.map((i) => i.path),
    );
  }
}

export const shelf = new Shelf();
