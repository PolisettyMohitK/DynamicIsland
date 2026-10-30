# Roadmap: iOS feel + Control Centre

Goal: stop feeling like Windows software. The island should feel like a thick, curved piece of
glass that moves like iOS, and it should become the fastest way to drive Windows.
Constraint all the way through: a still island costs nothing (no rAF loops, no polling at idle).

Reference studied: hitodama.in (30 Oct 2026). Their island is solid black; ours stays glass.

## Phase 1. Look and feel

**Hover flash (done).** At idle the glass captured a 30px strip once a second. On hover the island
grew at once but the capture loop could sleep up to 1s before grabbing a taller strip, so the glass
was mostly empty. A size change now wakes the loop immediately.

**Glass that looks thick, not plastic.** Today: 66% black tint, 3px blur, 1.5px flat rim, so it
reads as tinted see-through plastic.
- Less tint (~0.4), more blur (~8–12px), saturation ~1.4: the backdrop reads as bent light, not a dark film.
- A real bevel: a bright 1px outer line, a soft 8–12px inner light band lit from above, and a darker
  inner shade along the bottom edge. Together they read as the edge of a thick slab.
- Stronger edge refraction in a narrower band (lens squeezes the backdrop at the rim, like a
  rounded glass edge), slightly more colour fringing.
- A specular highlight that follows the cursor while hovered (one CSS variable on the rim layer,
  only while hovered, so idle stays free).
- Adaptive contrast: read the average brightness of each captured frame; over bright wallpaper,
  add a ~35% dimming layer (Apple's number) so text stays legible.
- Layered shadow (tight contact shadow + wide soft one) so it sits above the desktop.
- **Floats like iPhone** (decided): a capsule a few px below the top edge, round on every side,
  so the bevel and refraction show all the way round.

**Motion like iOS.**
- Real spring for the morph (bounce ~0.15) instead of a cubic-bezier, compiled to CSS `linear()`
  so it still runs on the compositor. Interrupting mid-morph keeps going smoothly.
- Content arrives with blur 4px → 0, scale 0.96 → 1, opacity, staggered 30–50ms (iOS-style
  "materialise"), and leaves faster than it arrives.
- Island leans a few px toward the cursor on hover and squishes slightly on press.
- Reduced motion: morph becomes a cross-fade (done: CSS movement is off).

**Idle clock.** While Choco sleeps in the pill: time on the left, day on the right
(`10:42 · (Choco) · Thu`). Digits roll when the minute changes; the colon breathes in time with
Choco's breathing beat; nothing animates between beats.

**Type and icons.** Segoe UI Variable Display for big numbers with tighter tracking, tabular
figures, one icon weight everywhere. (SF Pro can't ship on Windows: its licence is Apple-only.)

## Phase 2. Control Centre

Opens by scrolling down on the island (wheel or two-finger swipe). Tabs along the top, like
Hitodama's: Controls · Today · Stats · Clipboard · Apps · Wallpaper · Power.

| Tab | What | How on Windows |
|---|---|---|
| Controls | Wi-Fi, Bluetooth, audio output (with picker), dark mode, night light, volume + brightness sliders, now playing | WinRT `Radios` for Wi-Fi/Bluetooth; `IPolicyConfig` for output device; registry + `WM_SETTINGCHANGE` for dark mode; night light is an undocumented registry blob (may be fragile) |
| Today | Date, to-dos, water, focus, next reminder | Existing stores |
| Stats | CPU, GPU, RAM rings; top apps by RAM; disk, network, uptime; speed test | PDH counters (incl. GPU Engine); only polled while the tab is open |
| Clipboard | Recent text and images, click to paste, pin snippets | `AddClipboardFormatListener`, kept in memory + small file |
| Apps | Search apps, pinned and recent | Index Start Menu shortcuts once, refresh on change |
| Wallpaper | Pick from a folder with a carousel | `SystemParametersInfo(SPI_SETDESKWALLPAPER)` |
| Power | Lock, sleep, restart, shut down | `LockWorkStation`, `SetSuspendState`, `ExitWindowsEx`; restart/shut down ask first |

## Phase 3. Choco powers (from Hitodama)

- **Dictation**: hold a hotkey anywhere and talk; let go and clean text is typed at the cursor
  (Moonshine on-device, a quick LLM tidy for "uh, nine… no, ten", `SendInput` unicode).
- **Circle to ask**: hold a hotkey, draw around anything on screen, ask out loud. A transparent
  overlay records the lasso; the crop goes to Haiku with the question.
- Hotkeys are unregistered in game mode so nothing fires mid-match (decided).
- Not doing: routines / scheduled tasks.

## Order

1. Glass bevel + tint, spring morph, idle clock (one pass, tuned live in glass-lab).
2. Control Centre shell + Controls + Power (most daily value).
3. Stats, Apps, Clipboard, Wallpaper, Today.
4. Dictation, then circle to ask.
