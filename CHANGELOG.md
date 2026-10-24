# Changelog

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
