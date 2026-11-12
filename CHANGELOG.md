# Changelog

## 1.6.0 — 2026-11-12
- New (Beta): layout **3D (Beta)** — Agent Studio as a real **server rack**. Every project is a rack unit (in its color, with version, branch and open to-dos); the green LED is power, the blue one blinks while Claude works on the project, amber means it waits for you, red that its folder is missing. Apps and document projects have their own section. A click (or Tab + Enter) opens the project, "To the app" (or Esc) shows the normal window, the button "3D view" at the bottom right brings the rack back. Drag turns the room, the wheel zooms, Shift + drag moves it. Choose it in Settings → Appearance → Layout.

## 1.5.0 — 2026-11-07
- New: **connectors, skills & plugins** — a new right "Use connectors, skills & plugins" (project setup, routines, tasks). With it a run gets your Claude Code plugins that are turned on (with their skills, commands and agents), your MCP servers (`~/.claude.json`, the project's `.mcp.json`, the plugins' own) and your claude.ai connectors, and may use their tools and skills. Agent Studio's rights still are the only ones that count. A claude.ai connector is learned in the first run that sees it (that run asks you for approval). Without the right, auto mode uses no MCP tool.
- New: **two runs never change the same file at once.** The first run that changes a file holds it until it ends; another run that wants to change it is stopped with a note and works on something else first (or names the file as open). Done by a hook Agent Studio gives every Claude run (Edit, Write, MultiEdit, NotebookEdit); changes made through shell commands and ChatGPT runs are not covered.
- New: **Bit**, Agent Studio's mascot — a small round robot on the home page that shows at a glance what is going on: waiting (blinks), Claude working (bobs, antenna glows), something needs you (raised brows, "!" — a click opens "Needs you"), runs done today (smiles) or a problem. Also on the empty home page. Settings → General → "Show Bit on the home page"; still with "reduce motion".
- New: **projects that are no code** — Setup → "Kind of project": *App / code* or *Documents & more* (texts, school, research, notes …). A folder without code becomes a documents project by itself. It has its own kinds of work — Write, Revise, Research, Summarize, Organize, Feedback, Question — no Commands tab, and Changes only with git; Claude is told it works with texts and files, not code.
- Fixed: the project tile in the sidebar is square again (it was stretched to the width of the row) and shows the working ring there too (it was cut off).
- New: the commands and skills of your plugins in the `/` menu (`/plugin:command`), and Claude Code's own commands that work without a window: `/init`, `/code-review` (`/review`), `/security-review`, `/simplify`, `/context`, `/recap`, `/mcp`.
- New: more of Claude Code's commands in the chat box, done by Agent Studio: `/effort` (low … max, as `--effort`), `/output-style` (Explanatory, Learning), `/add-dir` (one more folder) — each for the next messages, shown as a chip; `/status`, `/permissions` (what is allowed and refused), `/agents`, `/skills`, `/plugin`, `/export` (the conversation as Markdown), `/copy`, `/memory` (CLAUDE.md), `/rewind` (take back the last run's commits), `/resume`, `/todos`, `/config`, `/login`, `/logout`, `/doctor`, `/release-notes`, `/reload-plugins`.

## 1.4.0 — 2026-11-05
- New: the **usage limit** of your Claude plan on the home page — the current 5-hour session and the week (all models, and Opus/Sonnet when your plan has them), each as a bar with the percent used and when it resets, like `/usage` in Claude Code. Read with Claude Code's own sign-in (the token is only sent to Anthropic, never stored), refreshed every 5 minutes or with "Refresh".
- New: four more CLI themes with other accent colors — CLI Amber, CLI Cyan, CLI Magenta and CLI White (black, monospace, square, like CLI (Terminal), just not green). Choose them like the other themes (Settings → Appearance or Settings menu → Theme).
- New: Claude is told which project it works on — every session of a project (also questions and routines) gets the project's name, version, kind and folders, plus your own words from Setup → **About the project (for Claude)**, like a system prompt for that app.
- New: a project Claude is working on shows a ring in the project's color running around its tile (sidebar, overview, project page) instead of the blinking dot; it stays still with "reduce motion".
- New: own **abbreviation** (up to three characters on the tile) and an own **shortcut** per project that opens it from anywhere (Setup).

## 1.3.0 — 2026-11-05
- New: six themes — Midnight (navy, blue), Dusk (plum, violet), Graphite (neutral gray, orange), High contrast (black, white, yellow — easy to read), Sand (warm beige, terracotta) and Rosé (soft pink, raspberry).
- New: two layouts — **Focus** (roomy, no status bar, bars and panels without lines, the tool rail fades until you point at it) and **Mirrored** (panels on the left, tool rail on the right). Choose them in Settings → Appearance (the themes also in the Settings menu).

## 1.2.0 — 2026-11-05
- New: CLI theme and CLI layout. Theme "CLI (Terminal)": black, phosphor green, monospace, square corners. Layout "CLI": monospace font, compact, square and flat (no shadows) — goes with every theme. Choose them in Settings → Appearance (the theme also in the Settings menu). The layout "Custom" also gets the new option Font → Monospace.

## 1.1.0 — 2026-11-05
- New: own theme and layout for this app. Settings → Appearance → "Only for this app": the theme and layout you choose here stay with Agent Studio; "Like Studio Hub" (default) keeps it looking like all Studio apps. Also settable in Studio Hub 1.13 → Theme per app.

## 1.0.0 — 2026-11-05
Agent Studio is rebuilt for building apps with AI.
- New: **projects** — your apps, in the sidebar. A project is a folder (usually a git repository); Agent Studio reads its
  name, version and stack (Tauri, Rust, Node, Python, Go, Swift, web), its commands, its git state (branch, changes,
  commits not pushed, last tag) and its README / STATUS / CHANGELOG. Each project has its own rights for Claude
  (level, allowed commands, internet, auto mode, model, rules, more folders), its to-do list and its sessions.
- New: the project page with the box for the **kinds of work**: Feature, Fix a bug, Tests, Clean up, Code review,
  Release, Question — each with its own instructions for Claude; reviews and questions run read-only; "Plan first".
- New: **Commands** — build, test, start the app or any command, run by Agent Studio itself (no AI) with live output;
  when one fails, "Let Claude fix it" sends the command and the end of its output. Menu Project: Ctrl+B / Ctrl+Shift+T / Ctrl+Shift+R.
- New: **Changes** — what is not committed yet, with the diff; let Claude review it (read-only), commit it or push it.
- New: **New app with AI** — name, place, kind (Studio app, web app, desktop app, Rust, Python or Claude decides) and
  what it should do: Agent Studio makes the folder and Claude sets the app up (skeleton, README, CHANGELOG, TODO.md,
  first tests, git). A folder with several apps offers to add each as a project.
- New: Overview of all apps; routines belong to a project (its folders and rights); new routine template "Daily code review".
- Fix: an approval of a refused command line with several commands (`git add … && git commit … && git push`) now
  really lets it through — each part is allowed, since Claude Code checks them one by one. Approvals asked for with
  the request block now carry the exact command.
- Moved: the AI tools for school are in Plan Studio now (study sheet routine template, Plan Studio's entries, study plan
  and flashcards no longer go through Agent Studio).
- Your setup moves over by itself: every folder your linked to-do lists worked in becomes a project with their rights.

## 0.12.1 — 2026-11-03
- Fix (macOS): the app no longer asks again and again for access to your Documents folder (or Desktop, Downloads). The app is now signed as a whole, so macOS can remember your answer. After an update, macOS may ask once more.

## 0.12.0 — 2026-11-03
- New: MCP server. Started with `--mcp`, Agent Studio opens no window and lets Claude, ChatGPT and other MCP clients use it: `todo_read`, `todo_add`, `agent_status`, `send_task`, `task_result`. Markdown to-do lists can be added to but not ticked (ticking stays with the app, only for lists you allowed); a task sent from a client always runs with read rights and without internet, and goes through Settings → Control like tasks from the other apps. Setup: see README → MCP.

## 0.11.0 — 2026-10-30
- New: Layouts, next to the themes (Settings → Appearance; shared by all Studio apps, any layout goes with any theme). **Adobe** is the look so far; **Modern** has rounded, floating panels and more room; **Custom** is yours: panels and tool rail left or right, density, corners, floating panels, status bar.

## 0.10.1 — 2026-10-30
- ChatGPT: a failed run now shows Codex's own message instead of its log lines; "Reconnecting…" and other notes on the way
  no longer show as errors in the log. Checked against the real Codex CLI 0.166.0 (not signed in).

## 0.10.0 — 2026-10-30
- ChatGPT as the other AI: pick a ChatGPT model wherever you pick a model (a chat, a routine, a task, `/model chatgpt`, or
  as the standard). The work is done by OpenAI's Codex CLI with your ChatGPT account; Settings → ChatGPT finds it and signs in.
- Its rights come from Codex's sandbox: read-only, or changing files in the folders at most (it can read elsewhere too).
  Commits and pushes stay with you, even at higher rights; "never delete" and allowed commands are only told to the model.
- Changing the AI in a conversation brings the last step along as text.

## 0.9.0 — 2026-10-30
- To-do lists stay short: each list shows only its last 10 ticked items (last in the file), a button under the list shows
  the older ones and hides them again. Open items always show; a section with nothing to show disappears.

## 0.8.1 — 2026-10-30
- The "Full view" button fits again: in the narrow details panel its text no longer wraps out of the button. A long
  task title is shortened with "…" instead (the whole title shows on hover); the ⤢ sign sits centered next to the text.

## 0.8.0 — 2026-10-24
- Strongly linked with the other Studio apps. Markdown, PDF, Data, Slides and Plan Studio get "Send to Agent Studio": a
  file or a selection with a task (e.g. proofread, summarize, make a quiz, explain the data, slides from an outline).
  - Agent Studio takes such tasks at once (it is started when it isn't open) and runs them as a chat, marked with the
    app's name in Chats and History. The answer and the files Claude made go back to the app as a card in the corner.
  - Rights: reading, or changing files in the file's folder when the app asks for it — never commit, push or delete.
    Settings → Control → "Tasks from other Studio apps": take them or not, at most read / change files, internet.
  - Questions and approvals come to "Needs you" as always; the app's card says that Agent Studio needs you.
- Agent Studio keeps a status for the other apps (what needs you, what runs, what was done today): Studio Hub shows it on
  the Agent Studio card, Plan Studio as a morning overview in Today.
- How it works: the apps and Agent Studio share the folder Studio/agent next to the shared settings (inbox, outbox,
  status.json); the code is the same file in every app (agentlink.rs / agentlink.js).

## 0.7.0 — 2026-10-24
- Normal chats outside of the to-dos, like in the Claude desktop app: the new view **Chats** (Alt+5, Agent → New chat,
  Ctrl+K) starts with a big box "How can I help?" — Enter sends, Shift+Enter makes a new line.
  - No folder needed: without one Claude works in a folder of its own in the app's data folder; or pick a folder in the
    box. Rights (read by default), internet (on by default) and the model are chosen right in the box and remembered.
  - The chat opens as a conversation: your messages as bubbles, Claude's answers with a little Markdown (code blocks,
    `code`, **bold**), "Claude is thinking …" with the live steps while it works, and "Steps" to look at them later.
    The chat box below goes on in the same conversation; slash commands work there too (`/clear` starts anew).
  - A chat's answers stay in the chat — they don't come to "Needs you" as "done" notes; approvals and problems still do.
  - Recent chats are listed under the box (newest first, with the number of messages); a click opens a chat at its
    newest message. Chats are marked in the history.
- Claude is told it is in a normal conversation (asks back in its answer, no summary at the end).

## 0.6.0 — 2026-10-24
- Slash commands like in the Claude desktop app: type `/` in a run's chat box (full view and panel) or press the `/`
  button — a list of commands opens; ↑/↓ choose, Tab or Enter take one, Esc closes it.
  - `/clear` (`/new`, `/reset`) starts a new conversation with the same rights and folders, `/model opus|sonnet|haiku|default`
    changes the model from the next message on, `/plan` only plans the next step, `/compact` summarises the
    conversation, `/cost` shows what it has cost so far, `/diff`, `/stop`, `/full` and `/help`.
  - What `/clear`, `/model` and `/plan` set shows as chips above the message (✕ removes it); `/clear text` and
    `/plan text` send right away.
  - Your own commands and skills: `~/.claude/commands/*.md` (subfolders as `folder:name`), `~/.claude/skills/*/SKILL.md`
    and the same in `.claude/` of the run's folders (a project's one wins). A command is sent as its text with
    `$ARGUMENTS` and `$1`…`$9` filled in, a skill as its instructions.
  - `//` at the start sends a text that begins with `/`.

## 0.5.0 — 2026-10-24
- A chat box like in the Claude desktop app in the full view: pinned to the bottom of the window, rounded, it grows with
  the text (up to 40 % of the window) and keeps its height and cursor when the run is redrawn. Enter sends, Shift+Enter
  makes a new line; the round button sends — or stops the run while Claude is working (you can already type ahead).
  The rights, auto mode and model of the conversation are shown in the box; it gets the focus when the full view opens.
- In the side panel Ctrl+Enter sends the message to Claude.

## 0.4.0 — 2026-10-24
- Tasks in full view: a run fills the whole window instead of the side panel — View → Task in full view (Alt+Enter),
  a double-click on a run (Needs you, to-dos, history), "Full view" in the panel or ⤢ on a card in "Needs you". The
  log in full height, summary and answer buttons on top, the chat below, details, changed files and commits on the
  side. A message, an answer or "Run again" stays in full view and follows to the new run. Esc or "Back" closes it.
- A message to Claude being typed is no longer lost when the run's details are redrawn.

## 0.3.0 — 2026-10-24
- New to-dos right in Agent Studio: each linked list has a field to add an item — pick the section (the file's headings,
  e.g. "Medium priority"; the last one used is remembered) and press Enter. The item goes into the file at the end of
  that section, in the list style used there. "Add & do it" (Ctrl+Enter) lets Claude start on it right away;
  Agent → New to-do (Ctrl+T) jumps to the field.
- Ticking finds the item again when lines moved meanwhile (e.g. after adding to-dos above it).

## 0.2.0 — 2026-10-24
- Auto mode (per routine, per list, per task): Claude works on its own within the rights — any command, no question for
  each step. Claude Code checks every step itself and still refuses risky ones (they come to "Needs you"); the rights
  stay the limit (e.g. "change files" never commits or pushes in auto mode), never force-push.
- Several to-do lists at once: pick more than one file when linking.
- Own folders and rights per list ("Folders & rights" on each list), e.g. one list per project; lists without them use
  the settings for all lists.

## 0.1.1 — 2026-10-24
- New app icon: the tile now fades from dark at the top left to lighter at the bottom right, like the other Studio apps.

## 0.1.0 — 2026-10-24
- First version (Pre-Alpha). Claude works for you with routines, your Markdown to-do list or on request — through
  Claude Code with your own sign-in; no keys or passwords are stored.
- "Needs you": questions, approvals (exactly what was refused), done notes and problems in one place, with a number on
  the app icon and notifications; answering goes on in the same conversation.
- Linked to-do lists: any Markdown file with checkboxes, "Do it" per item or "Work through" the list, the state of each
  item, ticking only when you allow it.
- Routines with templates: by button, daily, Mon–Fri, weekly, at start or when the to-do list changes; missed times are
  caught up.
- Rights per run (read · change files · commit · push), folders, allowed commands, internet, never delete, time limit,
  model, plan only; never force-push, reset --hard, clean or sudo.
- Live log, emergency stop, history with summary, changed files, commits, diff and undo of unpushed commits; rules for
  every run, a daily limit.
