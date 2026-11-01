<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { glassStrip, islandRect, makeLensMap, readGlassSettings, type Dock, type Rect, type Shape } from '$lib/glass';

  // Keep only red, green or blue (with alpha).
  const CHANNELS = [
    '1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0',
    '0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 1 0',
    '0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 1 0',
  ];

  let {
    shape,
    dock,
    gap,
    active,
    frameMs = 33,
    prewarm = [],
  }: {
    shape: Shape;
    dock: Dock;
    /** How far the island sits off the screen edge. */
    gap: number;
    active: boolean;
    /** Time between captures of what's behind the island. */
    frameMs?: number;
    /** Other shapes and gaps it'll take, to build their lenses early. */
    prewarm?: [Shape, number][];
  } = $props();

  let winW = $state(innerWidth);
  let winH = $state(innerHeight);

  const settings = readGlassSettings();
  const maps = new Map<string, string>();
  const mapFor = (s: Shape, g: number) => {
    const key = `${dock} ${g} ${s.w}x${s.h}x${s.r} ${winW}x${winH}`;
    if (!maps.has(key)) maps.set(key, makeLensMap(dock, g, winW, winH, s, settings.bezel, glassStrip(dock, winW, winH, s)));
    return maps.get(key)!;
  };
  const map = $derived(mapFor(shape, gap));
  // Only this strip of the window is captured, bent and drawn.
  const strip = $derived(glassStrip(dock, winW, winH, shape));

  let canvas: HTMLCanvasElement;
  // Read by the capture loop on every frame without restarting it.
  let grab: Rect = { x: 0, y: 0, width: 1, height: 1 };
  let captureEvery = 33;
  // Cuts the loop's sleep short: the idle pill sleeps ~1s, and a growing island must not wait
  // that long for a strip big enough to fill it.
  let wake = () => {};
  $effect(() => {
    grab = strip;
    captureEvery = frameMs;
    wake();
  });

  // Build the other lens maps ahead of time so the first expand doesn't stutter.
  $effect(() => {
    const id = requestIdleCallback(() => prewarm.forEach(([s, g]) => mapFor(s, g)));
    return () => cancelIdleCallback(id);
  });

  // The lens is baked into the canvas once per captured frame rather than set as a CSS filter:
  // a CSS filter re-runs on every frame the island draws, so anything moving on top (Choco's
  // face) would pay for the whole lens each frame.
  let frame: ImageBitmap | null = null;
  /** Where in the window `frame` was captured. */
  let frameAt: Rect = grab;
  /** 0–1: how much extra dimming the backdrop needs for white text to stay legible. */
  let dim = $state(0);
  // Average brightness behind the island from a sparse grid of the captured pixels.
  function measure(px: Uint8ClampedArray, at: Rect) {
    const r = islandRect(dock, gap, winW, winH, shape);
    const left = Math.max(0, Math.round(r.x - at.x));
    const top = Math.max(0, Math.round(r.y - at.y));
    const right = Math.min(at.width, left + shape.w);
    const bottom = Math.min(at.height, top + shape.h);
    let sum = 0;
    let n = 0;
    for (let y = top; y < bottom; y += 6) {
      for (let x = left; x < right; x += 12) {
        const i = (y * at.width + x) * 4;
        sum += 0.2126 * px[i] + 0.7152 * px[i + 1] + 0.0722 * px[i + 2];
        n++;
      }
    }
    const lum = n ? sum / n / 255 : 0;
    dim = Math.min(1, Math.max(0, (lum - 0.35) / 0.3));
  }
  function draw() {
    if (!frame) return;
    const ctx = canvas.getContext('2d')!;
    const css = getComputedStyle(canvas);
    ctx.filter = `url(#island-lens) saturate(${css.getPropertyValue('--glass-saturation')}) brightness(${css.getPropertyValue('--glass-brightness')})`;
    ctx.clearRect(0, 0, winW, winH);
    ctx.drawImage(frame, frameAt.x, frameAt.y);
  }
  // A new shape bends the last frame through its own lens.
  $effect(() => {
    map;
    draw();
  });

  // Stream what's behind the island while it's open; stop and clear when it isn't.
  $effect(() => {
    const ctx = canvas.getContext('2d')!;
    if (!active) {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      frame = null;
      return;
    }
    let alive = true;
    (async () => {
      let force = true; // the canvas starts empty, so the first frame must always come back
      while (alive) {
        const started = performance.now();
        try {
          const at = grab;
          const buf = await invoke<ArrayBuffer>('capture_backdrop', { ...at, force });
          force = false;
          if (alive && buf.byteLength) {
            const px = new Uint8ClampedArray(buf);
            measure(px, at);
            const next = await createImageBitmap(new ImageData(px, at.width));
            frame?.close();
            frame = next;
            frameAt = at;
            draw();
          }
        } catch (e) {
          console.error('capture_backdrop failed', e);
          await new Promise((r) => setTimeout(r, 500));
        }
        await new Promise<void>((r) => {
          const id = setTimeout(r, Math.max(0, captureEvery - (performance.now() - started)));
          wake = () => {
            clearTimeout(id);
            force = true; // a bigger strip is needed even if the screen hasn't changed
            r();
          };
        });
        wake = () => {};
      }
    })();
    return () => {
      alive = false;
      wake();
    };
  });

  // A highlight that follows the cursor across the glass. It only moves while the pointer does.
  let shine: HTMLDivElement;
  let queued = false;
  function onMove(e: PointerEvent) {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      const box = shine.getBoundingClientRect();
      shine.style.setProperty('--mx', `${e.clientX - box.left}px`);
      shine.style.setProperty('--my', `${e.clientY - box.top}px`);
      shine.classList.add('on');
    });
  }
</script>

<svelte:window onpointermove={onMove} bind:innerWidth={winW} bind:innerHeight={winH} />
<svelte:document onpointerleave={() => shine.classList.remove('on')} />

<svg width="0" height="0" aria-hidden="true">
  <filter
    id="island-lens"
    filterUnits="userSpaceOnUse"
    x={strip.x}
    y={strip.y}
    width={strip.width}
    height={strip.height}
    color-interpolation-filters="sRGB"
  >
    <feImage href={map} x={strip.x} y={strip.y} width={strip.width} height={strip.height} preserveAspectRatio="none" result="map" />
    <feGaussianBlur in="SourceGraphic" stdDeviation={settings.blur} edgeMode="duplicate" result="soft" />
    <!-- Bend red, green and blue by slightly different amounts for colour fringing, then recombine. -->
    {#each CHANNELS as matrix, c}
      <feDisplacementMap
        in="soft"
        in2="map"
        scale={settings.refraction * (1 + (c - 1) * settings.dispersion)}
        xChannelSelector="R"
        yChannelSelector="G"
        result="bent{c}"
      />
      <feColorMatrix in="bent{c}" type="matrix" values={matrix} result="ch{c}" />
    {/each}
    <feComposite in="ch0" in2="ch1" operator="arithmetic" k2="1" k3="1" result="rg" />
    <feComposite in="rg" in2="ch2" operator="arithmetic" k2="1" k3="1" />
  </filter>
</svg>

<!-- The canvas covers the whole window, lined up with it whatever the island's size. -->
<canvas
  bind:this={canvas}
  class="backdrop {dock}"
  width={winW}
  height={winH}
  style="width: {winW}px; height: {winH}px"
></canvas>
<!-- Flush, the tint (and the shine) run on under the corners that seep out into the edge. -->
<div class="tint {dock}" class:seep={gap <= 0}></div>
<div class="dim {dock}" class:seep={gap <= 0} style="opacity: {dim}"></div>
<!-- Flush against the screen edge, the bevel runs on past it, so that side has no lip. -->
<div class="bevel {dock}" class:flush={gap <= 0}></div>
<div class="shine {dock}" class:seep={gap <= 0} bind:this={shine}></div>

<style>
  svg {
    position: absolute;
  }
  .backdrop {
    position: absolute;
    top: calc(-1 * var(--gap));
    left: 50%;
    transform: translateX(-50%);
  }
  .backdrop.left,
  .backdrop.right {
    top: 50%;
    transform: translateY(-50%);
  }
  .backdrop.left {
    left: calc(-1 * var(--gap));
  }
  .backdrop.right {
    left: auto;
    right: calc(-1 * var(--gap));
  }
  .tint {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(to bottom, rgba(255, 255, 255, var(--glass-sheen)), rgba(255, 255, 255, 0) 40%),
      color-mix(in srgb, var(--glass-tint) calc(var(--glass-tint-alpha) * 100%), transparent);
    border-radius: inherit;
  }
  /* Apple's dimming layer for clear glass over bright content: 35% black, faded in by `dim`. */
  .dim {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    transition: opacity 400ms var(--ease-out);
  }
  /* Thickness, with no hard outline: light glowing in soft from every edge, brightest along the
     top, with a shaded band where the slab curves away underneath. */
  .bevel {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow:
      inset 0 14px 20px -12px rgba(255, 255, 255, calc(var(--glass-specular) * 0.55)),
      inset 0 -16px 22px -12px rgba(0, 0, 0, 0.45),
      inset 0 0 var(--glass-thickness) rgba(255, 255, 255, calc(var(--glass-specular) * 0.22));
    pointer-events: none;
  }
  /* Flush, it fades out before the screen edge, so the sides flow on into the corners seeping
     out there. */
  .bevel.flush {
    mask: linear-gradient(var(--side), transparent 24px, #000 calc(24px + 2 * var(--ear)));
  }
  .bevel.flush.top {
    --side: to bottom;
  }
  .bevel.flush.left {
    --side: to right;
  }
  .bevel.flush.right {
    --side: to left;
  }
  .flush.top {
    top: -24px;
  }
  .flush.left {
    left: -24px;
  }
  .flush.right {
    right: -24px;
  }
  .seep.top {
    left: calc(-1px - var(--ear));
    right: calc(-1px - var(--ear));
  }
  .seep.left,
  .seep.right {
    top: calc(-1px - var(--ear));
    bottom: calc(-1px - var(--ear));
  }
  .shine {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: radial-gradient(
      140px circle at var(--mx, 50%) var(--my, 0),
      rgba(255, 255, 255, calc(var(--glass-specular) * 0.22)),
      transparent 70%
    );
    opacity: 0;
    transition: opacity 300ms var(--ease-out);
    pointer-events: none;
  }
  .shine:global(.on) {
    opacity: 1;
  }
</style>
