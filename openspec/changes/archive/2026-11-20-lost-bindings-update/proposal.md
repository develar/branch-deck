# Proposal

## Why

Robot 1137287182 (2026-11-20) failed to compile `ClaudePromptLaunchSupport.kt`: local commit IJAI-6702 removed a
private constructor property and an import with their last use, and `brd update` replayed it onto an upstream commit
that used both again in other hunks. git conflicted only on a nearby block; the agent resolved it, the removals merged
cleanly outside the markers, the update applied, and HEAD did not compile. The user pressed Land three minutes later.
The landing gate refuses such a branch now (`lost_binding`), but the update itself must not apply such a result.

## What Changes

- The check of an update that would apply a clean rebase reads the lost bindings of its replay: the copies of the
  local commits against the local commits (`trial_check::lost_bindings::replay_lost_bindings`, one `log`, summaries
  only of the files the upstream changed too), counted only while the file at the tip still lacks the binding.
- A finding is a conflict that moves nothing: `FirstConflict.lost_bindings` names each lost name with its use. The
  app's dialog says what is lost and where, without "Rebase in a terminal", and its prompt keeps each name in a fixup
  commit. No Update Anyway.
- `brd update` starts a resolve session at a lost-bindings round on the tip the check replayed; a session checks the
  same before every finish, so a round of git on the same commit comes first. The round's copies are the files at the
  tip, edited anywhere; `--continue` refuses a name still unbound (`lost_binding`, exit 4) unless `--keep-name`, and
  writes the edits as one new `fixup! <subject>` commit on top; `next` names `brd fold`. `state.json` is version 4.

Capabilities: `cli/update`; the check and the app's words are the change `lost-bindings-update-check`
(`repository/update-project`), archived in the same commit.
