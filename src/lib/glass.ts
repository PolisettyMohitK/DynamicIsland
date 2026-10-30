// Lens maps for the liquid glass. A map is an image the size of the island
// window whose red/green channels tell feDisplacementMap how far to shift each
// pixel. Near the island's left, right and bottom edges, pixels sample from
// further inside, which bends the background like the rim of a thick glass slab.

export type Shape = { w: number; h: number; r: number };

export type GlassSettings = { refraction: number; bezel: number; dispersion: number; blur: number };

export function readGlassSettings(): GlassSettings {
  const css = getComputedStyle(document.documentElement);
  const num = (name: string) => parseFloat(css.getPropertyValue(`--glass-${name}`));
  return { refraction: num('refraction'), bezel: num('bezel'), dispersion: num('dispersion'), blur: num('blur') };
}

/** Map for `shape` top-centred in a `winW`×`winH` window. Outside the shape the map is neutral. */
export function makeLensMap(winW: number, winH: number, shape: Shape, bezel: number): string {
  const { w, h, r } = shape;
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

      const X = x + 0.5 - left;
      const Y = y + 0.5;
      if (X < 0 || X > w || Y > h) continue;

      let d: number;
      let nx = 0;
      let ny = 0;
      if (Y > h - r && (X < r || X > w - r)) {
        // rounded bottom corner
        const cx = X < r ? r : w - r;
        const dx = X - cx;
        const dy = Y - (h - r);
        const len = Math.hypot(dx, dy) || 1;
        if (len > r) continue;
        d = r - len;
        nx = dx / len;
        ny = dy / len;
      } else {
        // straight edge; the top edge sits against the screen and isn't bent
        d = Math.min(X, w - X, h - Y);
        if (d === X) nx = -1;
        else if (d === w - X) nx = 1;
        else ny = 1;
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
