// Lens maps for the liquid glass. A map is an image the size of the strip of window the glass
// draws in, whose red/green channels tell feDisplacementMap how far to shift each
// pixel. Near the edges of the island, pixels sample from
// further inside, which bends the background like the rim of a thick glass slab.

export type Shape = { w: number; h: number; r: number };

export type GlassSettings = { refraction: number; bezel: number; dispersion: number; blur: number };

export function readGlassSettings(): GlassSettings {
  const css = getComputedStyle(document.documentElement);
  const num = (name: string) => parseFloat(css.getPropertyValue(`--glass-${name}`));
  return { refraction: num('refraction'), bezel: num('bezel'), dispersion: num('dispersion'), blur: num('blur') };
}

/** The screen edge the island is docked to. Matches `Dock` in src-tauri/src/island.rs. */
export type Dock = 'top' | 'left' | 'right';

/** Gap between the screen edge and the resting island, logical px. Open, it's flush. */
export const ISLAND_GAP = 8;

/** Flush against the screen edge, the island's corners there curve out into it, like it's
    seeping out of the edge: by this much of its corner radius. */
export const EAR = 1;
export const earOf = (shape: Shape) => Math.round(shape.r * EAR);

export type Rect = { x: number; y: number; width: number; height: number };

/** The strip of the window the glass draws in: from the top edge down past the island, across
    the window; on a side, the island's rows (and the corners that seep out above and below). */
export function glassStrip(dock: Dock, winW: number, winH: number, shape: Shape): Rect {
  if (dock === 'top') return { x: 0, y: 0, width: winW, height: Math.min(winH, ISLAND_GAP + shape.h) };
  const y = Math.max(0, Math.round((winH - shape.h) / 2) - earOf(shape));
  return { x: 0, y, width: winW, height: Math.min(winH - y, shape.h + 2 * earOf(shape)) };
}

/** Where `shape` sits in a `winW`×`winH` window: centred along the edge it's docked to,
    `gap` off it. */
export function islandRect(dock: Dock, gap: number, winW: number, winH: number, shape: Shape) {
  const { w, h } = shape;
  if (dock === 'top') return { x: (winW - w) / 2, y: gap, w, h };
  return { x: dock === 'left' ? gap : winW - gap - w, y: (winH - h) / 2, w, h };
}

/** Map for `shape` docked at `dock`, `gap` off the edge, in a `winW`×`winH` window, covering
    `strip` of it. Outside the shape the map is neutral. Floating, every edge bends; flush against
    the screen edge (gap 0), that side runs straight on off the screen and doesn't bend. */
export function makeLensMap(dock: Dock, gap: number, winW: number, winH: number, shape: Shape, bezel: number, strip: Rect): string {
  let { x: left, y: top, w, h } = islandRect(dock, gap, winW, winH, shape);
  left -= strip.x;
  top -= strip.y;
  const r = Math.min(shape.r, w / 2, h / 2);
  // Flush, the sides stop bending towards the screen edge, so they run on into the corners that
  // seep out there (which don't bend) without a seam.
  const fade = gap <= 0 ? earOf(shape) + bezel : 0;
  if (gap <= 0) {
    const past = r + bezel;
    if (dock === 'top') (top -= past), (h += past);
    else if (dock === 'left') (left -= past), (w += past);
    else w += past;
  }
  const canvas = document.createElement('canvas');
  canvas.width = strip.width;
  canvas.height = strip.height;
  const ctx = canvas.getContext('2d')!;
  const img = ctx.createImageData(strip.width, strip.height);
  const px = img.data;

  for (let y = 0; y < strip.height; y++) {
    for (let x = 0; x < strip.width; x++) {
      const i = (y * strip.width + x) * 4;
      px[i] = px[i + 1] = px[i + 2] = 128;
      px[i + 3] = 255;

      // Position from the shape's centre, folded into one quadrant.
      const X = x + 0.5 - left - w / 2;
      const Y = y + 0.5 - top - h / 2;
      const qx = Math.abs(X) - (w / 2 - r);
      const qy = Math.abs(Y) - (h / 2 - r);

      let d: number;
      let nx = 0;
      let ny = 0;
      if (qx > 0 && qy > 0) {
        // rounded corner
        const len = Math.hypot(qx, qy);
        if (len > r) continue;
        d = r - len;
        nx = (qx / len) * Math.sign(X);
        ny = (qy / len) * Math.sign(Y);
      } else {
        // straight edge
        d = r - Math.max(qx, qy);
        if (d < 0) continue;
        if (qx > qy) nx = Math.sign(X);
        else ny = Math.sign(Y);
      }
      if (d >= bezel) continue;

      const t = 1 - d / bezel;
      let mag = 1 - Math.sqrt(1 - t * t); // circular lens profile
      if (fade) {
        const fromEdge = dock === 'top' ? y + strip.y : dock === 'left' ? x + strip.x : winW - x - strip.x;
        const k = Math.min(1, fromEdge / fade);
        mag *= k * k * (3 - 2 * k);
      }
      px[i] = 128 - nx * mag * 127;
      px[i + 1] = 128 - ny * mag * 127;
    }
  }
  ctx.putImageData(img, 0, 0);
  return canvas.toDataURL();
}
