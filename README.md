# Agent Studio (Rust / Tauri)

> **Status: Pre-Alpha (0.1)** — the first version; things may still change.

Claude as your assistant, under your control — part of the Studio family. Claude works for you by **routines**, by your
**Markdown to-do list** or on request; you decide what it may do, approve what goes beyond, see every step live and can
stop everything at any time. The native shell is Rust ([Tauri 2](https://tauri.app)); the UI is plain HTML/JS in `ui/`,
with the same chrome and controls as the other Studio apps.

Unlike the other Studio apps it needs the internet and a Claude account: the work is done by
[Claude Code](https://claude.com/claude-code) (the `claude` program) with **your own sign-in**. Agent Studio stores no
API key and no password; routines, logs and history stay on your computer.

## Features (0.2.0)
- **Needs you:** one box for everything Claude needs from you — questions (with answer buttons or your own text),
  approvals (exactly what was refused, e.g. `git push origin main`; approve or refuse, with a note), "done" notes with a
  summary, and problems. A number on the app icon and in the window title, and a notification of the system. Your answer
  goes on in the same conversation.
- **Linked to-do lists:** any Markdown file with checkboxes (`- [ ] …`), e.g. your `TODO.md`, grouped by its headings.
  "Do it" per item or "Work through" the whole list; each item shows its state (running · needs you · done · problem).
  The file stays yours: Claude only ticks an item when you allowed it for that file. Changes are noticed at once.
  Link as many lists as you like (several at once), each with **its own folders and rights** or the ones for all lists.
  **New to-dos right in the app:** type one under a list, pick the section (e.g. "Medium priority") and press Enter —
  it is written into the file at the end of that section; "Add & do it" (Ctrl+Enter) lets Claude start on it at once.
- **Routines:** a standing task in your words, the folders, the rights and when — only by button, every day, Mon–Fri,
  every week, when Agent Studio starts, or when the to-do list changes. A missed time is caught up when the app opens.
  Templates: work through the to-do list, bug hunt (report only), check builds, weekly review, study sheet from notes,
  check dependencies.
- **Rights per run:** read · change files · commit · push & publish, the folders it may work in, allowed commands,
  internet yes/no, never delete, a time limit, the model, **plan only** (say what would be done, change nothing) and
  **auto mode** (Claude Code's own mode: no question for each step, it checks every step itself and still refuses risky
  ones; the level stays the limit).
  Anything beyond is refused at once and comes to "Needs you". Never allowed: force-push, `git reset --hard`,
  `git clean`, `sudo`. Only Agent Studio's rights count — other Claude Code settings are not used in runs.
- **See what Claude does:** a live log of every step (files, commands, what was refused), the **emergency stop** for
  everything, a history of every run with summary, changed files, commits and the **diff**, and **undo** of a run's
  commits (as long as they are not pushed).
- **A task in full view:** open any run across the whole window (View → Task in full view, Alt+Enter, a double-click on
  a run, or "Full view" in its panel) — the log in full height, the summary, changed files and commits beside it, the
  answer buttons when it needs you and the chat below; Esc or "Back" returns to where you were.
- **Talk to Claude:** go on with any run — a question, a correction, the next step — in the same conversation.
- **Rules** for every run ("commit messages in German", "keep the changelog") and per routine, a **daily limit** of
  minutes, the time Claude worked today in the status bar.
- Sign-in from the app (`claude auth login` opens your browser), Claude Code found by itself (installer, npm, Homebrew,
  the Claude desktop app) or set by hand.
- 4 app themes, settings dialog, customizable shortcuts, auto-hide bars, German/English (Claude answers in the app's language).

Setup: `config.json`, runs and the box: `state.json`, one log per run: `runs/<id>.jsonl` — in the app's data folder
(Windows: `%APPDATA%\com.agentstudio.app`, macOS: `~/Library/Application Support/com.agentstudio.app`).

## How a run works
Agent Studio starts `claude -p --output-format stream-json` in the first folder, with
`--permission-mode dontAsk` (`plan` for "plan only", `auto` for auto mode — then everything above the level is refused outright), `--setting-sources ""` and its own `--settings` (the allowed and
refused tools of the run), and tells Claude how to ask: a ```` ```agent-studio ```` block at the end of its answer. The
prompt goes to its input. Refused tool calls (`permission_denials`) and that block become items of "Needs you"; an
answer or an approval continues the session (`--resume`), an approval with exactly that one command allowed.

## Layout
```
ui/                 index.html, app.js; native.js, autohide.js, shortcuts.js, shared.js (the same in every Studio app)
src-tauri/src/      main.rs — commands; runner.rs — rights, arguments, reading Claude's output, git (changes, diff, undo);
                    runs.rs — runs, logs, the box, notifications; todo.rs — Markdown checkboxes; shared.rs — shared settings
src-tauri/i18n/     en.json, de.json — all texts
```

## Build
Install Rust and Node, then `npx @tauri-apps/cli@2 icon app-icon.png` once and `npx @tauri-apps/cli@2 dev`.
Release builds for Windows / macOS / Linux come from GitHub Actions when you push a tag like `v0.1.0`.
