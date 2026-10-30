<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Digits from '../Digits.svelte';
  import Gauge from '../Gauge.svelte';

  type Stats = {
    cpu: number;
    cpuName: string;
    cores: number;
    gpu: number | null;
    gpuName: string;
    vramUsed: number | null;
    vramTotal: number | null;
    ramUsed: number;
    ramTotal: number;
    diskUsed: number;
    diskTotal: number;
    down: number;
    up: number;
    uptime: number;
    top: { name: string; memory: number }[];
  };

  let s = $state<Stats | null>(null);

  // Sampled once a second while this tab is open; the backend lets go when it closes.
  $effect(() => {
    let alive = true;
    let id: ReturnType<typeof setTimeout>;
    const tick = async () => {
      const next = await invoke<Stats>('stats_read').catch(() => null);
      if (!alive) return;
      if (next) s = next;
      id = setTimeout(tick, 1000);
    };
    tick();
    return () => {
      alive = false;
      clearTimeout(id);
      invoke('stats_stop');
    };
  });

  const GB = 1024 ** 3;
  const gb = (b: number) => (b / GB).toFixed(b >= 100 * GB ? 0 : 1);
  const rate = (b: number) =>
    b >= 1024 ** 2 ? `${(b / 1024 ** 2).toFixed(1)} MB/s` : `${Math.round(b / 1024)} KB/s`;
  const uptime = (secs: number) => {
    const d = Math.floor(secs / 86400);
    const h = Math.floor((secs % 86400) / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return d ? `${d}d ${h}h` : `${h}h ${m}m`;
  };
  /** Drop the marketing: "AMD Ryzen 7 5800H with Radeon Graphics" → "Ryzen 7 5800H". */
  const short = (name: string) =>
    name.replace(/\(R\)|\(TM\)|\d+th Gen |NVIDIA |AMD |Intel |GeForce |Core |with .*$|\d+-Core Processor|CPU @.*$/gi, '').trim();

  const rings = $derived(
    s
      ? [
          { label: 'CPU', value: s.cpu / 100, text: `${Math.round(s.cpu)}%`, sub: short(s.cpuName) || `${s.cores} cores` },
          ...(s.gpu != null
            ? [{ label: 'GPU', value: s.gpu / 100, text: `${Math.round(s.gpu)}%`, sub: short(s.gpuName) }]
            : []),
          {
            label: 'RAM',
            value: s.ramUsed / s.ramTotal,
            text: `${Math.round((100 * s.ramUsed) / s.ramTotal)}%`,
            sub: `${gb(s.ramUsed)} / ${gb(s.ramTotal)} GB`,
          },
          ...(s.vramUsed != null && s.vramTotal
            ? [
                {
                  label: 'VRAM',
                  value: s.vramUsed / s.vramTotal,
                  text: `${Math.round((100 * s.vramUsed) / s.vramTotal)}%`,
                  sub: `${gb(s.vramUsed)} / ${gb(s.vramTotal)} GB`,
                },
              ]
            : []),
        ]
      : [],
  );
</script>

{#if s}
  <div class="rings" style="--n: {rings.length}">
    {#each rings as r (r.label)}
      <div class="ring">
        <Gauge value={r.value} size={64} stroke={5} accent={r.value > 0.85 ? 'var(--bad)' : 'var(--text)'}>
          <b><Digits value={r.text} /></b>
        </Gauge>
        <span class="label">{r.label}</span>
        <small>{r.sub}</small>
      </div>
    {/each}
  </div>

  <div class="cols">
    <div class="card">
      <span class="head">Using the most memory</span>
      {#each s.top as app (app.name)}
        <div class="line">
          <span class="name">{app.name}</span>
          <span class="num">{gb(app.memory)} GB</span>
        </div>
      {/each}
    </div>
    <div class="card">
      <div class="line">
        <span class="head">Disk</span>
        <span class="num">{gb(s.diskUsed)} / {gb(s.diskTotal)} GB</span>
      </div>
      <div class="bar"><span style="width: {(100 * s.diskUsed) / (s.diskTotal || 1)}%"></span></div>
      <div class="line">
        <span class="head">Down</span><span class="num">{rate(s.down)}</span>
      </div>
      <div class="line">
        <span class="head">Up</span><span class="num">{rate(s.up)}</span>
      </div>
      <div class="line">
        <span class="head">Up for</span><span class="num">{uptime(s.uptime)}</span>
      </div>
    </div>
  </div>
{:else}
  <p class="empty">Reading…</p>
{/if}

<style>
  .rings {
    display: grid;
    grid-template-columns: repeat(var(--n), 1fr);
    gap: 8px;
  }
  .ring {
    min-width: 0;
    padding: 12px 6px 10px;
    border-radius: 18px;
    background: var(--pill);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .ring b {
    font-size: 13px;
    font-weight: 600;
  }
  .label {
    margin-top: 6px;
    font-size: 12px;
    font-weight: 600;
  }
  .ring small {
    max-width: 100%;
    font-size: 11px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .card {
    padding: 10px 14px;
    border-radius: 18px;
    background: var(--pill);
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
  }
  .head {
    font-size: 11.5px;
    color: var(--text-dim);
  }
  .card > .head {
    margin-bottom: 1px;
  }
  .name {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .num {
    flex: none;
    font-variant-numeric: tabular-nums;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--track);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--text);
  }
  .empty {
    margin: 0;
    padding: 30px 0;
    text-align: center;
    font-size: 12px;
    color: var(--text-dim);
  }
</style>
