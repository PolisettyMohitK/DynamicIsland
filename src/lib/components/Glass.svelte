<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { ISLAND_TOP, makeLensMap, readGlassSettings, type Shape } from '$lib/glass';

  // Must match WINDOW_WIDTH / WINDOW_HEIGHT in src-tauri/src/island.rs.
  const WIN_W = 900;
  const WIN_H = 640;
  // Keep only red, green or blue (with alpha).
  const CHANNELS = [
    '1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0',
    '0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 1 0',
    '0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 1 0',
  ];

  let {
    shape,
    active,
    frameMs = 33,
    prewarm = [],
  }: {
    shape: Shape;
    active: boolean;
    /** Time between captures of what's behind the island. */
    frameMs?: number;
    prewarm?: Shape[];
  } = $props();

  const settings = readGlassSettings();
  const maps = new Map<string, string>();
  const mapFor = (s: Shape) => {
    const key = `${s.w}x${s.h}x${s.r}`;
    if (!maps.has(key)) maps.set(key, makeLensMap(WIN_W, WIN_H, s, settings.bezel));
    return maps.get(key)!;
  };
  const map = $derived(mapFor(shape));

  let canvas: HTMLCanvasElement;
  // Read by the capture loop on every frame without restarting it.
  let captureHeight = 0;
  let captureEvery = 33;
  // Cuts the loop's sleep short: the idle pill sleeps ~1s, and a growing island must not wait
  // that long for a strip tall enough to fill it.
  let wake = () => {};
  $effect(() => {
    // The strip runs from the top of the screen to the island's bottom edge.
    captureHeight = ISLAND_TOP + shape.h;
    captureEvery = frameMs;
    wake();
  });

  // Build the other lens maps ahead of time so the first expand doesn't stutter.
  $effect(() => {
    const id = requestIdleCallback(() => prewarm.forEach(mapFor));
    return () => cancelIdleCallback(id);
  });

  // The lens is baked into the canvas once per captured frame rather than set as a CSS filter:
  // a CSS filter re-runs on every frame the island draws, so anything moving on top (Choco's
  // face) would pay for the whole lens each frame.
  let frame: ImageBitmap | null = null;
  /** 0–1: how much extra dimming the backdrop needs for white text to stay legible. */
  let dim = $state(0);
  // Average brightness behind the island from a sparse grid of the captured pixels.
  function measure(px: Uint8ClampedArray) {
    const left = Math.round((WIN_W - shape.w) / 2);
    let sum = 0;
    let n = 0;
    for (let y = ISLAND_TOP; y < ISLAND_TOP + shape.h; y += 6) {
      for (let x = left; x < left + shape.w; x += 12) {
        const i = (y * WIN_W + x) * 4;
        if (i + 2 >= px.length) break;
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
    ctx.clearRect(0, 0, WIN_W, WIN_H);
    ctx.drawImage(frame, 0, 0);
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
      ctx.clearRect(0, 0, WIN_W, WIN_H);
      frame = null;
      return;
    }
    let alive = true;
    (async () => {
      let force = true; // the canvas starts empty, so the first frame must always come back
      while (alive) {
        const started = performance.now();
        try {
          const buf = await invoke<ArrayBuffer>('capture_backdrop', { height: captureHeight, force });
          force = false;
          if (alive && buf.byteLength) {
            const px = new Uint8ClampedArray(buf);
            measure(px);
            const next = await createImageBitmap(new ImageData(px, WIN_W));
            frame?.close();
            frame = next;
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
            force = true; // a taller strip is needed even if the screen hasn't changed
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

<svelte:window onpointermove={onMove} />
<svelte:document onpointerleave={() => shine.classList.remove('on')} />

<svg width="0" height="0" aria-hidden="true">
  <filter
    id="island-lens"
    filterUnits="userSpaceOnUse"
    x="0"
    y="0"
    width={WIN_W}
    height={WIN_H}
    color-interpolation-filters="sRGB"
  >
    <feImage href={map} x="0" y="0" width={WIN_W} height={WIN_H} preserveAspectRatio="none" result="map" />
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

<canvas bind:this={canvas} class="backdrop" width={WIN_W} height={WIN_H}></canvas>
<div class="tint"></div>
<div class="dim" style="opacity: {dim}"></div>
<div class="bevel"></div>
<div class="shine" bind:this={shine}></div>
<div class="rim"></div>

<style>
  svg {
    position: absolute;
  }
  .backdrop {
    position: absolute;
    top: calc(-1 * var(--island-top));
    left: 50%;
    transform: translateX(-50%);
    width: 900px;
    height: 640px;
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
  /* Thickness: light caught along the top lip and glowing up from the bottom inside face,
     with a shaded band where the slab curves away underneath. */
  .bevel {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow:
      inset 0 1.5px 0.5px rgba(255, 255, 255, calc(var(--glass-specular) * 1.2)),
      inset 0 10px 16px -10px rgba(255, 255, 255, calc(var(--glass-specular) * 0.5)),
      inset 0 -1.5px 1px rgba(255, 255, 255, calc(var(--glass-specular) * 0.5)),
      inset 0 -12px 18px -10px rgba(0, 0, 0, 0.45),
      inset 0 0 var(--glass-thickness) rgba(255, 255, 255, calc(var(--glass-specular) * 0.18));
    pointer-events: none;
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
  /* Specular rim: a thin gradient border lit from --glass-light, brightest along the top. */
  .rim {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    padding: var(--glass-rim);
    background: linear-gradient(
      var(--glass-light),
      rgba(255, 255, 255, var(--glass-specular)) 0%,
      rgba(255, 255, 255, calc(var(--glass-specular) * 0.2)) 30%,
      rgba(255, 255, 255, calc(var(--glass-specular) * 0.08)) 55%,
      rgba(255, 255, 255, calc(var(--glass-specular) * 0.5)) 100%
    );
    mask:
      linear-gradient(#000 0 0) content-box,
      linear-gradient(#000 0 0);
    mask-composite: exclude;
    pointer-events: none;
  }
</style>
