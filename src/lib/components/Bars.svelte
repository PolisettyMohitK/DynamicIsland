<script lang="ts">
  // A small equaliser that dances while music plays and rests flat when it doesn't.
  let { playing, colour = 'var(--text)', size = 14 }: { playing: boolean; colour?: string; size?: number } = $props();
</script>

<span class="bars" class:playing style="--c: {colour}; --s: {size}px" aria-hidden="true">
  <i></i><i></i><i></i><i></i>
</span>

<style>
  .bars {
    width: var(--s);
    height: var(--s);
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
  }
  i {
    width: 2px;
    height: 100%;
    border-radius: 1px;
    background: var(--c);
    transform: scaleY(0.22);
    transition:
      transform 400ms var(--ease-out),
      background 400ms;
  }
  .playing i {
    animation: dance 900ms ease-in-out infinite alternate;
  }
  .playing i:nth-child(2) {
    animation-duration: 700ms;
    animation-delay: -300ms;
  }
  .playing i:nth-child(3) {
    animation-duration: 1000ms;
    animation-delay: -600ms;
  }
  .playing i:nth-child(4) {
    animation-duration: 800ms;
    animation-delay: -150ms;
  }
  @keyframes dance {
    0% {
      transform: scaleY(0.25);
    }
    50% {
      transform: scaleY(1);
    }
    100% {
      transform: scaleY(0.45);
    }
  }
</style>
