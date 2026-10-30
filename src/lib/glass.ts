// Lens maps for the liquid glass. A map is an image the size of the island
// window whose red/green channels tell feDisplacementMap how far to shift each
// pixel. Near every edge of the island, pixels sample from
// further inside, which bends the background like the rim of a thick glass slab.

export type Shape = { w: number; h: number; r: number };

export type GlassSettings = { refraction: number; bezel: number; dispersion: number; blur: number };

export function readGlassSettings(): GlassSettings {
  const css = getComputedStyle(document.documentElement);
  const num = (name: string) => parseFloat(css.getPropertyValue(`--glass-${name}`));
  return { refraction: num('refraction'), bezel: num('bezel'), dispersion: num('dispersion'), blur: num('blur') };
}

/** Gap between the top of the screen and the floating island, logical px. */
export const ISLAND_TOP = 8;

/** Map for `shape` centred, ISLAND_TOP down, in a `winW`×`winH` window. Outside the shape the
    map is neutral. Every edge bends: the island floats, so it's glass all the way round. */
export function makeLensMap(winW: number, winH: number, shape: Shape, bezel: number): string {
  const { w, h } = shape;
  const r = Math.min(shape.r, w / 2, h / 2);
  const canvas = document.createElement('canvas');
  canvas.width = winW;
  canvas.height = winH;
  const ctx = canvas.getContext('2d')!;
  const img = ctx.createImageData(winW, winH);
  const px = img.data;
  const left = (winW - w) / 2;

  for (let y = 0; y < winH; y++) {
    for (let x = 0; x < winW; x++) {
      const i = (y * winW + x) * 4;
      px[i] = px[i + 1] = px[i + 2] = 128;
      px[i + 3] = 255;

      // Position from the shape's centre, folded into one quadrant.
      const X = x + 0.5 - left - w / 2;
      const Y = y + 0.5 - ISLAND_TOP - h / 2;
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
