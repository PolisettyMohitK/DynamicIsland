// Lens maps for the liquid glass. A map is an image the size of the island
// window whose red/green channels tell feDisplacementMap how far to shift each
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

/** Where `shape` sits in a `winW`×`winH` window: centred along the edge it's docked to,
    `gap` off it. */
export function islandRect(dock: Dock, gap: number, winW: number, winH: number, shape: Shape) {
  const { w, h } = shape;
  if (dock === 'top') return { x: (winW - w) / 2, y: gap, w, h };
  return { x: dock === 'left' ? gap : winW - gap - w, y: (winH - h) / 2, w, h };
}

/** Map for `shape` docked at `dock`, `gap` off the edge, in a `winW`×`winH` window. Outside the
    shape the map is neutral. Floating, every edge bends; flush against the screen edge (gap 0),
    that side runs straight on off the screen and doesn't bend. */
export function makeLensMap(dock: Dock, gap: number, winW: number, winH: number, shape: Shape, bezel: number): string {
  let { x: left, y: top, w, h } = islandRect(dock, gap, winW, winH, shape);
  const r = Math.min(shape.r, w / 2, h / 2);
  if (gap <= 0) {
    const past = r + bezel;
    if (dock === 'top') (top -= past), (h += past);
    else if (dock === 'left') (left -= past), (w += past);
    else w += past;
  }
  const canvas = document.createElement('canvas');
  canvas.width = winW;
  canvas.height = winH;
  const ctx = canvas.getContext('2d')!;
  const img = ctx.createImageData(winW, winH);
  const px = img.data;

  for (let y = 0; y < winH; y++) {
    for (let x = 0; x < winW; x++) {
      const i = (y * winW + x) * 4;
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
      const mag = 1 - Math.sqrt(1 - t * t); // circular lens profile
      px[i] = 128 - nx * mag * 127;
      px[i + 1] = 128 - ny * mag * 127;
    }
  }
  ctx.putImageData(img, 0, 0);
  return canvas.toDataURL();
}
