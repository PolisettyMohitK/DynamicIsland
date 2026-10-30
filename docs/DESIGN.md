# Island — UI/UX spec

Interaction model studied from Nusa (screenshots taken 29 Oct 2026, 4K display at 125%).
We copy the *behaviour*; the look is our own: frosted glass instead of solid black.
All sizes below are logical px (Nusa's measured size ÷ 1.25).

## 1. States

| State | Size (w×h) | When | Content |
|---|---|---|---|
| **Idle** | ~100×30, attached to top edge | Nothing happening, mouse away | A small glass pill with Choco, a floppy-eared dog. Blinks and glances around; bobs along when music plays; sleeps (closed eyes, a drifting z) at night (23:00–07:00) or when Hey Choco is off. Glass refreshes once a second. |
| **Peek** | ~190×28 | Passed through on the way in or out of every other state | Status ring · time (bold) · audio bars. |
| **Live bar** | ~390×36 | Focus timer running, or music playing with lyrics turned on (the timer wins) | Left icon (♪ / timer) · one line centered (current lyric, title — artist, or "Focus time") · countdown on the right for the timer. Album-colour (or coral) glow. Its glass refreshes at 10fps instead of 30 to stay cheap. |
| **Expanded** | ~545×180 | Mouse hovers the notch | Header row + one page (see §3) + page dots. |
| **Expanded wide** | ~675×180 | Lyrics view open | Player on the left, lyrics panel on the right, divided by a thin line. |
| **Pop-up** | ~425×36, auto-hides after ~1.2s (reminders ~4s) | Volume/brightness/Caps Lock/battery/network/output device change; timer done, task due, water reminder | Icon left · label · level bar (volume, brightness) · value right (e.g. "On" in green). A reminder that arrives while the island is open shows when the cursor leaves. |
| **Voice** | ~320×44 | After "Hey Choco", while listening and transcribing | Choco with ears up (they twitch with your voice) · a soft white wave of your voice · "Listening", then what it heard. While thinking the wave becomes a slow ripple. No glow. Takes over from any other state. |
| **Answer** | 460 wide (540 for long replies or a card) × as tall as it needs, up to ~520 | Choco has acted or replied | Choco's face · what you said (faint) · the reply, words drifting in one by one; "- " lines are bullets. Long replies scroll inside with a soft fade. A live card (tasks, water, timer, music) can follow. Stays ~3s plus reading time per word (max 30s); hovering holds it. Afterwards Choco listens ~4s for a follow-up without the wake word (ears up, small wave). |
| **Settings** | ~720 wide × as tall as it needs | Gear in the expanded header | Three quiet columns: Choco (on/off, how easily it hears you, microphone, follow-ups, OpenCode key) · Focus and Water lengths/goals · Look (Soft or Mono colour, lyrics in the live bar) and System (start with Windows). |

- The shape is always **attached to the top edge**: flat top, rounded bottom corners (~32px when expanded, fully round when live bar).
- Moving between states is one continuous morph (width, height, radius animate together). Exact timings in §1b.
- Mouse leaves → collapses immediately back to Live bar or Idle.
- The OS window resizes with the island so nothing gets clipped and clicks pass through everywhere else.
- **Zero idle cost**: nothing animates or polls the GPU when the island is still.
- Hides automatically when a full-screen app or game is in front.

## 1b. Motion (measured from ~75fps frame captures)

| Transition | Timeline | Notes |
|---|---|---|
| **Hover → Expanded** | 0–90ms nothing (hover intent) · ~100ms Peek appears · holds Peek until ~330ms · 340–450ms morph to full size · content fades in during the last ~80ms | Two-stage: the Peek acts as a "glance" before committing. Moving away during Peek cancels cleanly. |
| **Mouse leaves → Idle** | 0–80ms shrink + content fades out · ~130ms Peek · gone by ~200ms | No delay before collapsing; collapse is faster than expand. |
| **Page switch** | 0–250ms new page slides in from the side it came from (~40px) while fading in; old page slides out + fades · settles by ~430ms | Ease-out (fast start, long soft settle). Header row and island size stay fixed. Active page dot slides to its new spot. |
| **Lyrics open/close** | ~350ms | Island widens evenly from the centre, no bounce. Album-colour glow rises from the bottom. Header row, arrows, page dots and volume fade out; player compacts to the left; lyrics column + divider fade in on the right. Reverse on close. |
| **Pop-up** | ~100ms Peek · ~220ms full bar · hold ~1.2s · ~1.45s back through Peek · gone ~1.6s | Same morph engine as hover. A new event while showing just swaps the content and restarts the hold. |

Implementation notes:
- One spring-style easing for size changes (ease-out with no visible overshoot), separate fade timing for content.
- Content never stretches: it cross-fades between states while the shape morphs underneath.
- Album art is circular in lyrics mode, rounded square otherwise.

## 2. Header row (expanded, shared by all pages)

`[page title]     [status ring] Thu 29 Oct  00:36(bold)     [audio bars]`

- Page title top-left in bold white, with a coloured second word (e.g. Todo**List**, Water **Reminder**).
- Status ring: battery/charging indicator, small circle with an icon.
- Date regular, time bold.
- Audio-bars icon on the right shows music is playing (static when paused).

Navigation: `‹` and `›` circular buttons on the left/right edges, page dots centred at the bottom (active dot is a wider pill). Sideways scroll (or Shift + wheel) also switches pages; scrolling down opens the Control Centre (§4g).

## 3. Pages

1. **Now playing**: square album art (rounded) left; title (bold) + artist (grey); elapsed `01:39`, seek bar, remaining `-01:28`; row: lyrics button · prev · play/pause · next · volume.
   - Lyrics button → **Expanded wide**: synced lyrics on the right, current line highlighted, click a line to seek.
   - Background picks up a **glow from the album-art colour** rising from the bottom edge.
   - Source: Windows media sessions (Spotify, browsers, any player). Lyrics: lrclib.net.
2. **Pomodoro**: large ring icon in accent colour on the left; big `25:00` + reset; label "Focus time"; buttons: Short Break · ▶ (accent-filled pill) · Focus Time. Chime at the end of each session. While running, the Live bar shows the countdown.
3. **Water**: `0.0 / 1.5 L` large; "Today · 0% · Next 08:07"; progress bar; reminder toggle; Drink + Settings pills; circular gauge on the right.
4. **Shelf**: dashed drop zone "Drop files here / Drag out anytime · click to open or copy". Dropped files show as thumbnails and can be dragged back out.
5. **To-do**: `0/0` large + calendar button; "Task Done Today"; Add Task / Edit Task pills; task list card on the right ("No tasks." empty state). Tasks can have a due time and show a pop-up when due.

## 4. Visual language (ours)

- **Surface**: dark liquid glass, in the style of Apple's Liquid Glass. While the island is open, the backend captures the strip of screen behind it (~30fps, unchanged frames skipped; the island excludes itself from capture) and `Glass.svelte` draws it through an SVG lens filter: edge refraction, colour fringing and blur, then the dark tint, top sheen and a specular rim. Every value is a `--glass-*` variable in `src/lib/theme.css`; tune them visually at `/glass-lab/index.html` on the dev server. Side effect: the island doesn't appear in screenshots or screen recordings.
- **Text**: white primary, ~60% white secondary. Font: Segoe UI Variable; numbers in tabular figures.
- **Controls**: pill buttons with ~10% white fill; the primary action is a filled pill in the page's accent colour.
- **Accent colours per page**: soft and muted — music = album colour, focus = coral, water = blue, shelf = neutral, to-do = amber. Mono (Settings > Look) makes them all one quiet white. These go into one theme file so the whole look is easy to change.
- **No "AI" look**: no rainbow orbs, sparkles, gradients or purple glows. Choco is a small white line-art dog; voice is a plain white wave.
- **Icons**: one consistent line-icon set (no emoji).

## 4b. Choco (voice)

- **Wake word**: "hey choco", a small model of our own on top of openWakeWord's front-end, run on-device in Rust (`src-tauri/src/choco.rs`) with ONNX Runtime. Trained with `tools/wakeword/train_choco.py` from synthetic Piper voices; the result is `src-tauri/models/hey_choco.onnx`. The mic is open the whole time Choco is on. Turn it on or off in Settings or from the tray menu ("Hey Choco on/off").
- **Speech-to-text**: Moonshine base, on-device. A command ends after ~0.9s of silence, gives up after 5s without speech (4s for a follow-up), and stops at 20s.
- **Conversation**: the last few exchanges (within 2 minutes) go along with each question, so follow-ups make sense.
- **Brain**: plain commands are matched offline in `src/lib/commands.ts` (timer, tasks with "at 5 pm", water, play/pause/next, volume, mute, the time). Anything else goes to DeepSeek V4.1 Flash via OpenCode Go (`src-tauri/src/ask.rs`), which replies in a sentence and can call the same actions. Needs an OpenCode API key in `OPENCODE_API_KEY` or `opencode_key.txt` in the app's config folder.
- **Feedback**: island only, no spoken voice.
- Models (~68 MB) download to the app's local data folder on first use. The wake-word models are CC BY-NC-SA: fine for personal use, not for selling.

## 4c. Choco (pet)

- **Where it lives**: asleep in the pill. Hovering the island, "hey choco" or a reminder wakes it; it drops out and follows the cursor at a lazy distance (walk, then gallop when far). It goes home after the idle time in Settings, at night (23:00–07:00), on double-click, or when a game starts.
- **Window**: a small transparent, click-through window that moves with the dog (`src-tauri/src/pet.rs`, page `src/routes/pet`). Made on first wake, hidden at home and kept for 2 min, then closed.
- **Motion** (after oneko and vscode-pets): an alert beat before it sets off, a skid when it pulls up, then it settles stand → sit → the odd trick → yawn → lie down. Tricks are weighted by energy and never repeat back to back. Each act plays once and stops, so a still dog draws nothing.
- **Reactions**: hover 1s = happy; click = bark or hop; music = head bob; focus timer = lies down quietly; voice = ears up; reminder = runs to the cursor.
- **Settings → Pet**: on/off, distance, energy, sleep after 1–10 min, night sleep, sounds.

## 4d. Game mode

- A full-screen app (or a borderless window covering its monitor) for 1s puts the app to sleep: the island hides, its WebView2 is suspended, the mic closes, the pet goes home and its window closes, pollers slow to 2s. It wakes 2s after the game leaves the screen, where it was.
- Measured on a release build: ~0.07% CPU while gaming. RAM drops (~420 → ~330 MB private) but not to zero.

## 4e. Choco (computer use)

- **When**: the model behind Choco decides a request needs the screen ("open Spotify and play lofi") and calls `use_computer`. No separate mode to turn on.
- **How** (`src-tauri/src/computer.rs`): Claude Haiku 5.5 via OpenCode Go's Anthropic endpoint with the `computer_toolset_20260801` tool. The primary screen goes out as a 1280-wide PNG; clicks, typing, keys and scrolling go back in through `SendInput`, scaled to real pixels. The island and the pet are excluded from capture, so they never show in its screenshots. At most 40 model turns.
- **Island**: a 440×44 bar with Choco thinking and a few words on what it's doing, plus "Esc to stop"; then the usual answer.
- **Safety**: before sending, buying, deleting, posting or accepting terms it must ask; the bar shows the question with No / Yes (no answer in 60s = no). It never types passwords or payment details. Text on screen is treated as information, not instructions.
- **Stopping**: hold Esc, click "Esc to stop", say "hey choco" (anything), or start a game.
- **Thinking**: Haiku uses adaptive thinking, so it plans before acting and aims for what you meant rather than the literal words.

## 4f. Choco's memory

- Short facts about you, one per line in `choco_memory.txt` in the app's config folder (`src-tauri/src/memory.rs`). Both models read them every time and can `remember` new ones; the voice brain can also `forget` one. Up to 60, oldest dropped first. Editable by hand.
- Say "remember that…", "what do you know about me?", "forget that…". Choco also saves lasting preferences it notices (the apps you use, your name). Never passwords, codes or payment details.

## 4g. Control Centre

- **Open**: scroll down on the island (hovered or expanded). Scroll up goes back to the pages; the cursor leaving collapses it after the usual 1s hold.
- **Controls**: tiles for Wi-Fi, Bluetooth and the output device (a list to pick from), dark mode and mute; volume and brightness sliders (brightness only on built-in panels); now playing. A switch's state lives in its round icon, lit white when on.
- **Today**: the date, open to-dos (tick them off), focus timer and water from the existing stores.
- **Wallpaper**: `Pictures\Wallpapers` then the Windows ones, two rows that scroll sideways; click to set. "Add your own" opens the folder. Thumbnails are shrunk with WIC in `wallpaper.rs`.
- **Stats**: CPU, GPU, RAM and VRAM rings, the apps using the most memory, disk, network rate, uptime. Sampled once a second only while the tab is open (`stats.rs`: sysinfo, PDH GPU counters like Task Manager's, DXGI for the GPU name).
- **Clipboard**: the last 30 texts copied, in memory only; skips anything its app marks private (password managers). Click one to paste it into the app in use (`clipboard.rs`).
- **Apps**: search the Start menu (`Get-StartApps`, read once), Enter opens the top hit; recent apps when the search is empty (`apps.rs`).
- **Power**: lock, sleep, sign out, restart, shut down. The last three need a second click within 3s.
- **How** (`src-tauri/src/control.rs`): WinRT radios, the Personalize registry keys plus a `WM_SETTINGCHANGE` broadcast, WMI brightness, the undocumented `IPolicyConfig` for the default output (what EarTrumpet uses), `shutdown.exe`. State is read when it opens; nothing polls.

## 4h. Dictation and Circle to ask

Both are off while a game runs, and both find their keys by polling every 40–50ms instead of a system-wide keyboard hook.
- **Dictation** (`dictation.rs`): hold Right Ctrl on its own for 300ms anywhere, talk, then let go. Pressing any other key while holding cancels it, so Ctrl shortcuts never trigger it. Moonshine transcribes on this machine, sharing the copy "Hey Choco" uses. If there's an OpenCode key, DeepSeek tidies the text (fillers, self-corrections, punctuation) with a 6s limit; without it, the raw text is used. The text is typed into the focused app with `SendInput`. The island shows a mic, the level, then what was typed. There's an on/off toggle in Settings.
- **Circle to ask** (`circle.rs`, `routes/circle`): Ctrl+Shift+Space opens a dimmed overlay on the monitor the cursor is on. You draw around something and the inside stays bright. The overlay listens for a question right away; typing takes over. The circled area is grabbed from the real screen (the overlay is excluded from capture) and sent to Claude Haiku 5.5 with the question. The answer appears in a card next to the circle, and Esc closes it.

## 5. Build order

1. Shell: states + morph + window resizing + acrylic + top-edge anchoring + hide on full-screen + tray menu.
2. Now playing + lyrics.
3. Pomodoro + to-do.
4. Pop-ups (volume, brightness, Caps Lock, battery, Wi-Fi, headphones).
5. Shelf.
6. Water reminder.
7. Choco voice assistant.
