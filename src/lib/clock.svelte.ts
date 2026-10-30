// The current time for anything on screen. Island.svelte ticks it only while
// the island is open, so the idle island does no work.
export const clock = $state({ now: Date.now() });
