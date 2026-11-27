# Proposal

## Why

"Start in Air" opened a dialog of its own: the agent, the model, the effort, the mode, Plan, Run or Draft and the
prompt. Branch Deck remembered the choice per repository and followed the started session in a toast. The New Session
composer of Air has all of these pickers already. The dialog was a second copy of it, and it went out of date with each
change of Air.

## What Changes

- "Start in Air" reads the fast prompt and sends `x-air-ij/openDraft` with the project and the prompt alone
  (`air_open_draft`). Air opens its New Session tab with the prompt and the default agent of the New Session button, and
  the user picks the rest there.
- A toast says "Opening in Air…" while the IDE answers, then "Opened in <IDE>", "No IDE with Air has <repository> open"
  with "Copy Prompt", or the refusal in its words.
- The dialog, the remembered choice (`airLaunch`), the targets, the launch of a session, the watch of its row and the
  session toast are gone, with the commands `air_launch_targets`, `air_launch`, `air_watch_session` and
  `stop_air_watch`.
- "Start in Air" opens no form under a row now: a review window runs it itself.

Capabilities: `branch/start-in-air`, `branch/patronus`, `branch/safe-push-failure-prompt`, `review/review-window`.
