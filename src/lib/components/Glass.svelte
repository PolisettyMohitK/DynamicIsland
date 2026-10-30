<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { makeLensMap, readGlassSettings, type Shape } from '$lib/glass';

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
  $effect(() => {
    captureHeight = shape.h;
    captureEvery = frameMs;
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
            const next = await createImageBitmap(new ImageData(new Uint8ClampedArray(buf), WIN_W));
            frame?.close();
            frame = next;
            draw();
          }
        } catch (e) {
          console.error('capture_backdrop failed', e);
          await new Promise((r) => setTimeout(r, 500));
        }
        await new Promise((r) => setTimeout(r, Math.max(0, captureEvery - (performance.now() - started))));
      }
    })();
    return () => {
      alive = false;
    };
  });
</script>

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
<div class="rim"></div>

<style>
  svg {
    position: absolute;
  }
  .backdrop {
    position: absolute;
    top: 0;
    left: 50%;
    transform: translateX(-50%);
    width: 900px;
    height: 640px;
  }
  .tint {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(to bottom, rgba(255, 255, 255, var(--glass-sheen)), rgba(255, 255, 255, 0) 45%),
      color-mix(in srgb, var(--glass-tint) calc(var(--glass-tint-alpha) * 100%), transparent);
    box-shadow: inset 0 0 14px rgba(255, 255, 255, calc(var(--glass-specular) * 0.12));
    border-radius: inherit;
  }
  /* Specular rim: a thin gradient border lit from --glass-light. No rim on the top edge,
     which sits against the screen. */
  .rim {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    padding: 0 var(--glass-rim) var(--glass-rim);
    background: linear-gradient(
      var(--glass-light),
      rgba(255, 255, 255, var(--glass-specular)) 0%,
      rgba(255, 255, 255, calc(var(--glass-specular) * 0.15)) 28%,
      rgba(255, 255, 255, 0) 50%,
      rgba(255, 255, 255, calc(var(--glass-specular) * 0.15)) 72%,
      rgba(255, 255, 255, calc(var(--glass-specular) * 0.55)) 100%
    );
    mask:
      linear-gradient(#000 0 0) content-box,
      linear-gradient(#000 0 0);
    mask-composite: exclude;
    pointer-events: none;
  }
</style>
