<script lang="ts">
  // The music as a springy wave behind Now Playing, from what the speakers play (waves.rs).
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import type { WaveStyle } from '$lib/ui.svelte';

  let { style, colour }: { style: WaveStyle; colour: string } = $props();

  // Each band chases its level on a spring, so the wave overshoots and settles.
  const STIFFNESS = 0.16;
  const DAMPING = 0.72;
  /** No levels for this long (nothing playing through the speakers) and the wave lies flat. */
  const QUIET_MS = 300;

  let canvas: HTMLCanvasElement;
  let w = $state(0);
  let h = $state(0);

  $effect(() => {
    let target: number[] = [];
    let pos: number[] = [];
    let vel: number[] = [];
    let heard = 0;
    let frame = 0;
    const unlisten = listen<number[]>('music-waves', (e) => {
      target = e.payload;
      heard = performance.now();
    });
    invoke('waves_start');

    const tick = () => {
      frame = requestAnimationFrame(tick);
      const quiet = performance.now() - heard > QUIET_MS;
      for (let i = 0; i < target.length; i++) {
        const goal = quiet ? 0 : target[i];
        vel[i] = ((vel[i] ?? 0) + (goal - (pos[i] ?? 0)) * STIFFNESS) * DAMPING;
        pos[i] = (pos[i] ?? 0) + vel[i];
      }
      draw(pos);
    };
    tick();
    return () => {
      cancelAnimationFrame(frame);
      unlisten.then((off) => off());
      invoke('waves_stop');
    };
  });

  function draw(levels: number[]) {
    const ctx = canvas?.getContext('2d');
    if (!ctx || !w || !h) return;
    const dpr = devicePixelRatio;
    if (canvas.width !== Math.round(w * dpr)) canvas.width = Math.round(w * dpr);
    if (canvas.height !== Math.round(h * dpr)) canvas.height = Math.round(h * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const n = levels.length;
    if (n < 2) return;
    // The ends ease down to nothing so the wave flows out of the island's sides.
    const lv = levels.map((l, i) => Math.max(0, Math.min(1, l)) * Math.sin((Math.PI * (i + 0.5)) / n));
    const x = (i: number) => (i / (n - 1)) * w;
    ctx.fillStyle = ctx.strokeStyle = `rgb(${colour})`;
    ctx.lineWidth = 2;
    ctx.lineCap = ctx.lineJoin = 'round';

    if (style === 'bars') {
      const bw = (w / n) * 0.45;
      lv.forEach((l, i) => {
        const bh = Math.max(2, l * h * 0.9);
        ctx.beginPath();
        ctx.roundRect(x(i) - bw / 2, h - bh, bw, bh, bw / 2);
        ctx.fill();
      });
    } else if (style === 'dots') {
      lv.forEach((l, i) => {
        ctx.beginPath();
        ctx.arc(x(i), h - 4 - l * (h - 8), 2.5, 0, Math.PI * 2);
        ctx.fill();
      });
    } else if (style === 'mirror') {
      const mid = h / 2;
      curve(ctx, lv.map((l, i) => [x(i), mid - l * mid * 0.9]));
      curve(ctx, lv.map((l, i) => [x(i), mid + l * mid * 0.9]).reverse(), true);
      ctx.closePath();
      ctx.globalAlpha = 0.35;
      ctx.fill();
      ctx.globalAlpha = 1;
      ctx.stroke();
    } else {
      const base = h * 0.8;
      curve(ctx, lv.map((l, i) => [x(i), base - l * base * 0.9]));
      ctx.stroke();
    }
  }

  /** A smooth line through the points, through the middle of each pair. */
  function curve(ctx: CanvasRenderingContext2D, pts: number[][], join = false) {
    if (join) ctx.lineTo(pts[0][0], pts[0][1]);
    else {
      ctx.beginPath();
      ctx.moveTo(pts[0][0], pts[0][1]);
    }
    for (let i = 1; i < pts.length - 1; i++) {
      const mx = (pts[i][0] + pts[i + 1][0]) / 2;
      const my = (pts[i][1] + pts[i + 1][1]) / 2;
      ctx.quadraticCurveTo(pts[i][0], pts[i][1], mx, my);
    }
    const last = pts[pts.length - 1];
    ctx.lineTo(last[0], last[1]);
  }
</script>

<canvas bind:this={canvas} bind:clientWidth={w} bind:clientHeight={h}></canvas>

<style>
  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
