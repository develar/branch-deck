# UX Writing: Toasts, Errors and Labels

**Status:** Active  
**Updated:** 2026-10-14  
**Tags:** [ui, wording, toasts]

## Summary

One vocabulary for the whole app. A toast says what happened, to what, in sentence case; its description is the detail;
an error says what could not be done and why; a toast about an object opens it. Buttons, menu items and toast actions
are Title Case; everything that reads as a sentence is sentence case. [Activity](../../spec/repository/activity.spec.md)
records the toasts in their words, so a toast and its entry take their text from one function.

## Context

Before this guide the toasts spoke three dialects: "Success / Branch X pushed successfully", "Pushed oauth" and
"`feature-auth`: Changes Amended"; errors were "Failed to …", "… Failed", "Can't …" and "Error"; some error toasts were
sticky and logged, others vanished after 5 s. The user read a toast, then could not find the thing it was about.

## Toasts

- **The title says what happened**, in the past tense, with its object: "Pushed feature-ui", "Amended 3 files into
  feature-auth", "Updated master: rebased 2 commits onto origin/master", "Safe Push started", "auth merged into master".
- **Never "Success" or "Error" as a title**: the icon already says it.
- **The description is the detail**: what the push did ("1 commit · created the remote branch", "1 commit · dropped 2
  remote commits"), what a robot tests ("air-session-runtime → master · profile Large Scale Change"), the amended commit.
  It never repeats the title.
- **No "branch: Title Case" prefixes** ("feature-auth: Changes Amended"): the branch is part of the sentence ("Unapplied
  feature-auth").
- **Name the object the user knows**: the virtual branch, never its ref (`develar/virtual/air-session-runtime`).
- **A toast about an object opens it** with its action:
  - a Patronus robot (started, restarted, canceling, merged): "Open in Patronus" (`robot.url`);
  - a pushed branch: "Open Pull Request", or "Create Pull Request" without one (GitHub);
  - a Space merge request: "Open Merge Request";
  - the installed Agent CLI: "Copy Path".
  A toast with an action stays 10 s (`ACTION_TOAST_TIMEOUT_MS`). A toast with two actions (the merged robot: Fetch,
  then "Open in Patronus") uses a custom body of `src/components/ui/toast.tsx` (`data.kind`), the actions below the
  text, the primary one first.
- **A precondition is `info`, not an error**: "No commits to unapply", "No changes to amend", "Nothing to change",
  "master is already up to date with origin/master". Nothing failed; nothing is logged.
- **No toast for feedback in place**: a copy button says "Copied" in place (`CopyButton`, `useCopyToClipboard`), the
  "Copy Prompt for Agent" button too; a menu item copies without a toast; an amend conflict opens its viewer below the
  row and says nothing else.

## Errors

- **Every error goes through `notifyError(title, error)`** (`src/lib/errors.ts`): the toast is sticky, the error is
  logged to the backend log and recorded in Activity. Never a hand-made `toast.add({ type: "error" })`;
  `withRowProcessing` takes an `errorTitle` and calls `notifyError` itself.
- **The title is "Cannot <verb> <object>"**: "Cannot push feature-ui", "Cannot start the Safe Push", "Cannot copy to the
  clipboard". One failure verb: not "Failed to …", "… Failed", "Can't …", "Couldn't …".
- **The description is the reason**, the `message` of the error (`getErrorDetails`), never prefixed with "Error:".
- The same words in a tooltip or an inline error: "Cannot check the update: <reason>".

## Words and punctuation

- **Title Case** for buttons, menu items, card titles, tabs and toast actions: "Create Pull Request", "Open in
  Patronus", "Copy Path", "Install and Restart".
- **Sentence case** for dialog titles, toast titles, descriptions, tooltips, placeholders and help heads.
- **"…"** (the ellipsis character, never "...") for progress ("Pushing…") and on every command that opens a form
  ("Safe Push…", "Unapply…").
- **"canceled"**, "canceling" (US spelling), "Branch Deck".
- **No exclamation marks**.
- **"·" is the only separator** between parts of a line ("3 commits · created the remote branch"); never "•", "|" or
  " - ".

## How to

- A success or info toast: `showToast({ title, description, action })` from `src/lib/toast-message.ts`
  (`type: "info"` for a precondition), or `toast.add` from `@/components/ui/toast` for a custom one (a loading toast, a
  custom body).
- A history rewrite with an Undo: `showUndoToast(title, undo)` from `src/lib/command.ts`.
- An inline form's action: `withRowProcessing(key, fn, { success, errorTitle })` or `withRowProgress` when the action
  toasts itself (`src/stores/inline-row-action.ts`).
- A toast that Activity records takes its words from the function the entry uses: `describePush`
  (`src/lib/push-summary.ts`), `describeRobot` (`src/lib/patronus.ts`, through `announceRobot` in
  `src/lib/patronus-actions.ts`), `appliedUpdateToast` (`src/lib/landed.ts`).

## References

- [UI Theme](../design/ui-theme.md): colours, states, the copy button.
- [Activity](../../spec/repository/activity.spec.md): the log in the words of the toasts.
