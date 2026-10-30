# Changelog

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
