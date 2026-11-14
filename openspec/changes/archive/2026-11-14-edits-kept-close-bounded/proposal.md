# Proposal

## Why

The review of "no edit is lost" found three more ways to lose or hold an edit: a field emptied and entered during the
write of a blur wrote nothing, as `useConfigDraft` compared it with the value shown, which is the value before the
write until it answers; a write that threw left `FieldWrites` holding a rejected run, so the field never wrote again;
and a close whose save got no answer kept the Settings window open.

## What Changes

- `FieldWrites` decides whether a save has anything to write (`save(value, shown, …)`): while a write runs, the value
  shown is no answer; its run ends in a `finally`, so a write that threw lets the next save write.
- `saveAllBeforeClose` waits at most `CLOSE_SAVE_LIMIT_MS` (3 s), then the window is destroyed, as after a failed save.
- Capability `repository/settings`: the scenarios of an edit back to the value shown, of an edit kept unsaved, and of
  a close whose save gets no answer.
