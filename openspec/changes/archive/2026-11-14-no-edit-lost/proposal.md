# Proposal

## Why

The review of "one change is one write" found that `FieldWrites` lost an edit made during a write: a save while a
write ran waited for it and wrote nothing, whatever its value, and the answer of the first write cleared the field, the
new typing with it. An edit typed and entered (or a close of the window) during the write of a blur wrote only the
value before. The prefix of every repository (Settings → Branches, All repositories) saved on Enter and on blur but not
when the window closed. The E2E helper that closes the window as an answer lands counted microtasks.

## What Changes

- `FieldWrites`: a save during a write writes its value after it, the latest of such saves, unless it is the value that
  write took or refused; `taken` names the value, and the field drops its typing only while it is that value.
- `BranchPrefixSetting` saves through `FieldWrites` and before the window closes (`saveBeforeClose`), waiting for the
  writer (`whenSettingWritten`).
- `closeAsTheAnswerLands` closes in a task posted as the body of the answer is read, before the render React posts after
  it, instead of after 200 microtasks.
- Capability `repository/settings`: no edit is lost. `repository/onboarding`: the prefix field saves when the window
  closes.
