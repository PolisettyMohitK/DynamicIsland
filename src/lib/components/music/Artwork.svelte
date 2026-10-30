<script lang="ts">
  import { nowPlaying } from '$lib/media.svelte';
  import Icon from '../Icon.svelte';

  let { size, round = false }: { size: number; round?: boolean } = $props();
</script>

<div class="art" class:round style="width: {size}px; height: {size}px">
  {#if nowPlaying.artwork}
    <img src={nowPlaying.artwork} alt="" />
  {:else}
    <Icon name="music" size={size * 0.4} />
  {/if}
</div>

<style>
  .art {
    flex: none;
    border-radius: 12px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--pill);
    color: var(--text-dim);
    transition: border-radius 350ms var(--ease-out);
  }
  .art.round {
    border-radius: 50%;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
