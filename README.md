# Agent Studio (Rust / Tauri)

> **Status: 1.0 — rebuilt for building apps with AI**; still young, things may change.

Build apps with AI — part of the Studio family. Everything is organized by **project**: the apps you build, each with
its own rights for Claude, its to-do list, its sessions with Claude, its commands (build, test, start) and its changes
(git). You decide what Claude may do, approve what goes beyond, see every step live and can stop everything at any time.
The native shell is Rust ([Tauri 2](https://tauri.app)); the UI is plain HTML/JS in `ui/`, with the same chrome and
controls as the other Studio apps. (The AI tools for school are in Plan Studio.)

Unlike the other Studio apps it needs the internet and a Claude account: the work is done by
[Claude Code](https://claude.com/claude-code) (the `claude` program) with **your own sign-in**. Agent Studio stores no
API key and no password; routines, logs and history stay on your computer.

## Features (1.0)
- **Projects** in the sidebar, an **Overview** of all apps (version, branch, open tasks, changes, what runs). Add a folder
  (Ctrl+O) — Agent Studio reads name, version, stack, commands, git state and README / STATUS / CHANGELOG; a folder with
  several apps offers to add each one. Setup per project: name, color, folder and more folders, to-do list, rules and
  the rights for Claude (level, allowed commands, internet, auto mode, model, never delete, time limit).
- **The project page**: a box with the kinds of work — **Feature, Fix a bug, Tests, Clean up, Code review, Release,
  Question** — each with its own instructions for Claude (reviews and questions read-only, "Plan first" on request);
  open tasks, sessions, commands, last commits and the documents beside it. Tabs: Overview, Tasks, Sessions, Commands,
  Changes, Setup (Ctrl+1 … Ctrl+5).
- **Commands**: build, test, start the app or any command — run by Agent Studio itself in the project folder (login
  shell), with live output; a failed one goes to Claude with "Let Claude fix it". Ctrl+B, Ctrl+Shift+T, Ctrl+Shift+R.
- **Changes**: what is not committed yet, with the diff; let Claude review it (read-only), commit it or push it.
- **New app with AI** (Ctrl+Shift+A): name, place, kind (Studio app, web app, desktop app, Rust, Python, or Claude
  decides) and what it should do — Agent Studio makes the folder, Claude sets the app up (skeleton, README, CHANGELOG,
  TODO.md, first tests, git with a first commit) in auto mode; pushing and publishing stay with you.
- **ChatGPT as the other AI:** a ChatGPT model can be picked wherever a model is picked (or `/model chatgpt`). The work is
  done by OpenAI's [Codex CLI](https://github.com/openai/codex) (`codex exec --json`) with your ChatGPT account; Settings → ChatGPT
  finds it and signs in. Rights come from Codex's sandbox with your own Codex config and rules ignored: read-only, or
  changing files in the folders at most (it can read elsewhere too). Commits and pushes stay with you; "never delete" and
  allowed commands are only told to the model, not enforced — use Claude where you need those guarantees.
- **Linked with the other Studio apps:** Markdown, PDF, Data, Slides and Plan Studio have **"Send to Agent Studio"**
  (Ctrl+Shift+A) — a file or a selection with a task (proofread, summarize, quiz, explain the data, slides from an
  outline …). It runs here as a chat marked with the app's name; the answer and the files Claude made go back to the
  app as a card (open them there with one click). Rights: read, or change files in the file's folder — never commit,
  push or delete (Settings → Control). Studio Hub shows what needs you on the Agent Studio card; Plan Studio has a
  morning overview in Today, homework and dates in your own words ("Maths p. 34 by Thursday") and study plan suggestions.
- **Chats** like in the Claude desktop app, outside of to-dos and routines: ask Claude anything in the big box of the
  Chats view (Ctrl+K) — no folder needed (or pick one), rights, internet and model right in the box. The chat is a
  conversation with bubbles and answers in a little Markdown, the steps on request, and goes on in the same session;
  its answers stay in the chat instead of "Needs you".
- **Needs you:** one box for everything Claude needs from you — questions (with answer buttons or your own text),
  approvals (exactly what was refused, e.g. `git push origin main`; approve or refuse, with a note), "done" notes with a
  summary, and problems. A number on the app icon and in the window title, and a notification of the system. Your answer
  goes on in the same conversation.
- **To-do lists per project:** any Markdown file with checkboxes (`- [ ] …`), e.g. your `TODO.md`, grouped by its headings. Only the last 10 ticked items of a list show; a button below it opens the rest.
  "Do it" per item or "Work through" the whole list; each item shows its state (running · needs you · done · problem).
  The file stays yours: Claude only ticks an item when you allowed it for that file. Changes are noticed at once.
  Each project has one list (e.g. its `TODO.md`), worked on with the project's folders and rights.
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
- **One run per file:** two runs never change the same file at the same time — the first one holds it until it ends,
  another one is stopped with a note and does something else first (Claude's file tools; not shell commands, not ChatGPT).
- **Bit, the mascot,** on the home page shows what is going on — waiting, working, needs you, done, a problem (can be
  turned off in Settings → General).
- **See what Claude does:** a live log of every step (files, commands, what was refused), the **emergency stop** for
  everything, a history of every run with summary, changed files, commits and the **diff**, and **undo** of a run's
  commits (as long as they are not pushed).
- **A task in full view:** open any run across the whole window (View → Task in full view, Alt+Enter, a double-click on
  a run, or "Full view" in its panel) — the log in full height, the summary, changed files and commits beside it, the
  answer buttons when it needs you and a chat box like in the Claude desktop app pinned to the bottom (Enter sends,
  Shift+Enter for a new line, the round button stops a running task); Esc or "Back" returns to where you were.
- **Talk to Claude:** go on with any run — a question, a correction, the next step — in the same conversation.
- **Slash commands** like in the Claude desktop app: `/` in the chat box lists them — `/clear`, `/model`, `/plan`,
  `/compact`, `/cost`, `/diff`, `/stop`, `/full`, `/help` — plus **your own commands and skills** from
  `~/.claude/commands`, `~/.claude/skills` and `.claude/` in the run's folders (sent as their text, since runs don't
  load Claude Code's own setup), the commands and skills of your plugins (`/plugin:command`), and Claude Code's own
  `/init`, `/review`, `/security-review`, `/context` and `/mcp`.
- **Connectors, skills & plugins** (a right per project, routine or task): the run gets your Claude Code plugins that
  are turned on, your MCP servers and your claude.ai connectors, and may use their tools and skills — still with only
  Agent Studio's rights. A connector not known yet asks for approval the first time.
- **Rules** for every run ("commit messages in German", "keep the changelog") and per routine, a **daily limit** of
  minutes, the time Claude worked today in the status bar.
- Sign-in from the app (`claude auth login` opens your browser), Claude Code found by itself (installer, npm, Homebrew,
  the Claude desktop app) or set by hand.
- 4 app themes, settings dialog, customizable shortcuts, auto-hide bars, German/English (Claude answers in the app's language).

Setup: `config.json`, runs and the box: `state.json`, one log per run: `runs/<id>.jsonl`, chats without a folder work in `chats/` — in the app's data folder
(Windows: `%APPDATA%\com.agentstudio.app`, macOS: `~/Library/Application Support/com.agentstudio.app`).

## How a run works
Agent Studio starts `claude -p --output-format stream-json` in the first folder, with
`--permission-mode dontAsk` (`plan` for "plan only", `auto` for auto mode — then everything above the level is refused outright), `--setting-sources ""` and its own `--settings` (the allowed and
refused tools of the run), and tells Claude how to ask: a ```` ```agent-studio ```` block at the end of its answer. The
prompt goes to its input. Refused tool calls (`permission_denials`) and that block become items of "Needs you"; an
answer or an approval continues the session (`--resume`), an approval with exactly that one command allowed.

## How the apps talk to Agent Studio
Through files in `Studio/agent/` next to the shared settings (macOS: `~/Library/Application Support/Studio/agent`,
Windows: `%APPDATA%\Studio\agent`): an app writes a task to `inbox/<id>.json` (`agentlink.rs`, the same file in every
app, with `agentlink.js` for the dialog and the result cards) and starts Agent Studio when its heartbeat is old.
Agent Studio takes it, starts a chat and keeps `outbox/<id>.json` up to date (status, answer, new files, and data from a
```` ```studio-result ```` block, e.g. Plan Studio's entries); the app removes it once seen. `status.json` holds what
needs you, what runs, what was done and a heartbeat.

## MCP
Agent Studio is also an MCP server, so Claude (Desktop or Code), ChatGPT and other MCP clients can use what it can do:
started with `--mcp`, the program opens no window and speaks MCP on stdin / stdout until the client closes it.

- `todo_read` — Reads a Markdown to-do list: its items (line, text, done, section, depth) and sections.
- `todo_add` — Adds a new item at the end of a section of a Markdown to-do list.
- `agent_status` — Whether Agent Studio is open, and what needs the person, runs and was done today.
- `send_task` — Sends a task to Agent Studio. It runs with read rights and without internet as soon as Agent Studio is open and takes tasks. Returns the task's id.
- `task_result` — How a task from send_task goes and what came out of it.

Markdown to-do lists can be added to but not ticked (ticking stays with the app, only for lists you allowed); a task sent from a client always runs with read rights and without internet, and goes through Settings → Control like tasks from the other apps. Files are given as full paths.

Claude Code: `claude mcp add agent-studio -- "/Applications/Agent Studio.app/Contents/MacOS/agent-studio" --mcp`.
Claude Desktop (`claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "agent-studio": {
      "command": "/Applications/Agent Studio.app/Contents/MacOS/agent-studio",
      "args": [
        "--mcp"
      ]
    }
  }
}
```
The program is `/Applications/Agent Studio.app/Contents/MacOS/agent-studio` on macOS, `agent-studio.exe` in the install folder on
Windows, and `agent-studio` (or the AppImage) on Linux.

## Layout
```
ui/                 index.html, app.js (runs, chats, routines, settings), projects.js (projects, their page, commands,
                    changes, new app); native.js, autohide.js, shortcuts.js, shared.js (the same in every Studio app)
src-tauri/src/      main.rs — commands; project.rs — what a project is (stack, commands, git); jobs.rs — running
                    commands with live output; runner.rs — rights, arguments, reading Claude's output, git (changes, diff, undo);
                    runs.rs — runs, logs, the box, notifications; todo.rs — Markdown checkboxes; slash.rs — own slash commands and skills;
                    shared.rs — shared settings; agentlink.rs — the link to the other apps (the same in every app);
                    link.rs — Agent Studio's side of it: tasks in, results out, status.json
src-tauri/i18n/     en.json, de.json — all texts
```

## Build
Install Rust and Node, then `npx @tauri-apps/cli@2 icon app-icon.png` once and `npx @tauri-apps/cli@2 dev`.
Release builds for Windows / macOS / Linux come from GitHub Actions when you push a tag like `v0.1.0`.
