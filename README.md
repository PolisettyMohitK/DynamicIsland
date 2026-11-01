<div align="center">

<img src="src-tauri/icons/128x128@2x.png" width="128" alt="Choco" />

# Islet

A floating island for Windows, made of liquid glass, with a small dog called Choco living in it.

Tauri 2 · Svelte 5 · Rust

</div>

---

## What it does

The island sits at the top of the screen as a small glass pill. Choco sleeps in it, blinks, and bobs along when music plays. Hover it and it opens up.

- **Now playing**: album art, seek bar, controls, and synced lyrics (from lrclib.net) that you can click to seek. Works with Spotify, browsers, and any player Windows knows about.
- **Focus timer**: 25/5 sessions with a live countdown bar while it runs.
- **Water**: daily goal, cups, and reminders.
- **Shelf**: drop files on the island to park them; drag them back out anywhere.
- **To-do**: today's tasks, with due times that pop up when they're due.
- **Pop-ups**: volume, brightness, Caps Lock, battery, network and output device changes.
- **Liquid glass**: the island bends and blurs what's behind it in real time. It hides itself from screen captures.

### Choco

- **"Hey Choco"**: an on-device wake word and speech-to-text. Plain commands ("start a focus session", "add a task to call mum at 5", "next song") run offline and instantly. Anything else goes to an LLM, which can answer and use the same actions.
- **Uses your computer**: "hey choco, open Notepad and type hello". When a request needs the screen, Choco looks at it and clicks and types for you. It asks Yes / No on the island before anything risky (sending, buying, deleting, posting). Hold Esc or say "hey choco" to stop it.
- **Remembers you**: tell it your name, the apps you use, what you like. It keeps short facts and uses them every time. "What do you know about me?" and "forget that…" work too.
- **Desktop pet**: Choco hops out of the island and follows your cursor, sits, does tricks and naps, then goes home.
- **Game mode**: a full-screen game puts the whole app to sleep (about 0.07% CPU) and it wakes up where it was when you leave.

## Running it

Needs Windows 10 or 11, [Node 20+](https://nodejs.org), [pnpm](https://pnpm.io) and [Rust](https://rustup.rs).

```bash
pnpm install
pnpm tauri dev        # run with hot reload
pnpm tauri build      # release build
```

The first time Choco turns on, it downloads its speech models (~68 MB) into the app's local data folder.

### AI key

Choco's offline commands work without a key. For questions, computer use and memory it needs a key for one of:

- [OpenCode](https://opencode.ai) (Go plan): DeepSeek V4.1 Flash answers, Claude Haiku 5.5 uses the computer.
- [OpenAI](https://platform.openai.com/api-keys): `gpt-6-luna` answers, `gpt-6.1-terra` uses the computer.
- [Gemini](https://aistudio.google.com/apikey): `gemini-3.8-flash` for both.

Paste keys into the dashboard (it opens with the app, or from the tray), pick which service to use, and change the models if you like. Keys are stored in the app's config folder, never in the repo; `OPENCODE_API_KEY`, `OPENAI_API_KEY` and `GEMINI_API_KEY` work too.

## Layout

| Where | What |
|---|---|
| `src/lib/components/Island.svelte` | The island: states, morphing, pages |
| `src/lib/components/Glass.svelte` | The liquid glass lens |
| `src/lib/choco.svelte.ts`, `src/lib/commands.ts` | Choco's states and what it does with what it hears |
| `src-tauri/src/island.rs` | The window: placement, hit area, hover, full-screen detection |
| `src-tauri/src/choco.rs` | Microphone, wake word and speech-to-text |
| `src-tauri/src/ask.rs` | Choco's brain (LLM) |
| `src-tauri/src/computer.rs` | Computer use |
| `src-tauri/src/memory.rs` | What Choco knows about you |
| `src-tauri/src/pet.rs` | The desktop pet |
| `src-tauri/src/game.rs` | Game mode |
| `tools/wakeword/` | Training the "hey choco" model |
| `docs/DESIGN.md` | The full design: states, sizes, timings, behaviour |

## Third-party pieces

- [openWakeWord](https://github.com/dscripka/openWakeWord) front-end models (downloaded at runtime). Its models and data are CC BY-NC-SA 4.0, and `src-tauri/models/hey_choco.onnx` is trained on top of them, so the wake word is for non-commercial use.
- [Moonshine](https://github.com/usefulsensors/moonshine) base for speech-to-text (MIT).
- [lrclib.net](https://lrclib.net) for lyrics.
- [OpenCode Go](https://opencode.ai), [OpenAI](https://openai.com) or [Gemini](https://ai.google.dev) for the brain and computer use, whichever you pick.

## Credits

Designed and built by **Polisetty Mohit K**, with [Claude](https://claude.com/claude-code).

Contributions from Hemanth Varma Kanumuri (local AI, the dashboard and settings window, and fixes).

It started as a fork of [thanvish21/dynamic-island](https://github.com/thanvish21/dynamic-island), but has since been rewritten and none of that code remains.

## License

MIT © 2026 Polisetty Mohit K and contributors. See [LICENSE](LICENSE).
