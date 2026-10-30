// What's playing, shared by every part of the island that shows music.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type Media = {
  title: string;
  artist: string;
  album: string;
  app: string;
  playing: boolean;
  positionMs: number;
  durationMs: number;
  updatedAtMs: number;
};

export type LyricLine = { t: number; text: string };
type LyricsState = 'loading' | 'ready' | 'none';

const keyOf = (m: Media) => `${m.title}\u0000${m.artist}\u0000${m.album}`;

class NowPlaying {
  media = $state<Media | null>(null);
  artwork = $state<string | null>(null);
  /** Album colour as "r g b", for glows and accents. */
  colour = $state<string | null>(null);
  lyrics = $state<LyricLine[]>([]);
  lyricsState = $state<LyricsState>('none');
  /** Bumped by the island's ticker while it's visible; drives the position. */
  now = $state(Date.now());

  #key = '';
  #lyricsCache = new Map<string, LyricLine[]>();

  /** Playback position in ms, extrapolated between updates from Windows. */
  get position() {
    const m = this.media;
    if (!m) return 0;
    const p = m.playing ? m.positionMs + (this.now - m.updatedAtMs) : m.positionMs;
    return Math.max(0, m.durationMs > 0 ? Math.min(p, m.durationMs) : p);
  }

  /** Index of the lyric line being sung, or -1 before the first line. */
  get line() {
    const t = this.position;
    let i = -1;
    while (i + 1 < this.lyrics.length && this.lyrics[i + 1].t <= t) i++;
    return i;
  }

  start() {
    const off = listen<Media | null>('media', (e) => this.#update(e.payload));
    invoke<Media | null>('media_current').then((m) => this.#update(m));
    return () => off.then((f) => f());
  }

  togglePlay() {
    // Flip right away so the button responds instantly; Windows confirms within ~0.5s.
    if (this.media) {
      this.media = { ...this.media, playing: !this.media.playing, positionMs: this.position, updatedAtMs: Date.now() };
    }
    invoke('media_play_pause');
  }

  next() {
    invoke('media_next');
  }

  previous() {
    invoke('media_previous');
  }

  seek(ms: number) {
    if (this.media) this.media = { ...this.media, positionMs: ms, updatedAtMs: Date.now() };
    invoke('media_seek', { positionMs: ms });
  }

  #update(m: Media | null) {
    this.media = m;
    this.now = Date.now();
    const key = m ? keyOf(m) : '';
    if (key === this.#key) return;
    this.#key = key;
    this.#loadArtwork(key);
    if (m) this.#loadLyrics(m, key);
    else {
      this.lyrics = [];
      this.lyricsState = 'none';
    }
  }

  async #loadArtwork(key: string) {
    if (this.artwork) URL.revokeObjectURL(this.artwork);
    this.artwork = null;
    this.colour = null;
    if (!key) return;
    const bytes = await invoke<ArrayBuffer>('media_artwork');
    if (key !== this.#key || !bytes.byteLength) return;
    const url = URL.createObjectURL(new Blob([bytes]));
    this.artwork = url;
    const colour = await albumColour(url).catch(() => null);
    if (key === this.#key) this.colour = colour;
  }

  async #loadLyrics(m: Media, key: string) {
    const cached = this.#lyricsCache.get(key);
    if (cached) {
      this.lyrics = cached;
      this.lyricsState = cached.length ? 'ready' : 'none';
      return;
    }
    this.lyrics = [];
    this.lyricsState = 'loading';
    let lines: LyricLine[] = [];
    try {
      const lrc = await invoke<string | null>('fetch_lyrics', {
        title: m.title,
        artist: m.artist,
        album: m.album,
        durationMs: m.durationMs,
      });
      lines = lrc ? parseLrc(lrc) : [];
      this.#lyricsCache.set(key, lines); // only cache real answers, not network errors
    } catch (e) {
      console.error('fetch_lyrics failed', e);
    }
    if (key !== this.#key) return;
    this.lyrics = lines;
    this.lyricsState = lines.length ? 'ready' : 'none';
  }
}

export const nowPlaying = new NowPlaying();

/** "[01:23.45] words" lines -> sorted {t (ms), text}. Lines with several tags repeat. */
export function parseLrc(lrc: string): LyricLine[] {
  const out: LyricLine[] = [];
  for (const raw of lrc.split(/\r?\n/)) {
    const tags = [...raw.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)];
    if (!tags.length) continue;
    const text = raw.replace(/\[[^\]]*\]/g, '').trim();
    for (const [, min, sec] of tags) out.push({ t: (+min * 60 + +sec) * 1000, text: text || '♪' });
  }
  return out.sort((a, b) => a.t - b.t);
}

/** A vivid, readable colour from the album art, as "r g b". */
async function albumColour(url: string): Promise<string | null> {
  const img = new Image();
  img.src = url;
  await img.decode();
  const size = 24;
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext('2d')!;
  ctx.drawImage(img, 0, 0, size, size);
  const px = ctx.getImageData(0, 0, size, size).data;

  // Average the pixels, weighting colourful ones far above greys.
  let r = 0, g = 0, b = 0, total = 0;
  for (let i = 0; i < px.length; i += 4) {
    const max = Math.max(px[i], px[i + 1], px[i + 2]);
    const min = Math.min(px[i], px[i + 1], px[i + 2]);
    const weight = ((max - min) / 255) ** 2 * (max / 255) + 0.001;
    r += px[i] * weight;
    g += px[i + 1] * weight;
    b += px[i + 2] * weight;
    total += weight;
  }
  if (!total) return null;
  // Keep the hue, but lift it to a consistent brightness so it glows on dark glass.
  const [h, s] = rgbToHsl(r / total, g / total, b / total);
  const [R, G, B] = hslToRgb(h, Math.max(s, 0.55), 0.58);
  return `${R} ${G} ${B}`;
}

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b), l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h / 6, s, l];
}

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s, p = 2 * l - q;
  const f = (t: number) => {
    t = (t + 1) % 1;
    const v = t < 1 / 6 ? p + (q - p) * 6 * t : t < 1 / 2 ? q : t < 2 / 3 ? p + (q - p) * (2 / 3 - t) * 6 : p;
    return Math.round(v * 255);
  };
  return [f(h + 1 / 3), f(h), f(h - 1 / 3)];
}
