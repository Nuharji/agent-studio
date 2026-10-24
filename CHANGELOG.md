# Changelog

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
