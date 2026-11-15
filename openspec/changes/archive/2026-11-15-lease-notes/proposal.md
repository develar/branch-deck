# Proposal

## Why

Agents keep logs, probes and patches in `<lease>/notes.local/`, and brd knew nothing of it. Before, `notes.local`
survived the return, so the next holder of the slot read stale logs as its own; since a clean rule deleted it at the
return, the probes of a reviewer and the gate logs the lead still needed were lost, and agents copied them to `/tmp` by
hand.

## What Changes

- Each lease has notes of its own outside its worktree, beside the state of the pool:
  `$GIT_COMMON_DIR/branch-deck/notes/<lease id>/` and a record (`<lease id>.json`). The get links `notes.local` of the
  worktree to them and names them (`notes`, and `brd lease notes --lease-id <id>` in `next`); the return removes the
  link and keeps them, so the next holder of the slot starts empty.
- The link is no change of the worktree: the get adds `/notes.local` to `info/exclude`, and the status of a slot never
  counts an untracked `notes.local`.
- `brd lease notes [--lease-id <id> | --holder <name>] [--list]` names the notes of a held or returned lease and lists
  their files; `brd lease status` and the return name them; `brd lease disk` counts them.
- The notes of a returned lease expire after `branchdeck.leaseNotesKeep` (14 days; `0` for ever) through `brd lease
  trim` and the keeper of a host, never while the lease is held.
- Where the system makes no link (Windows without the right), `notes.local` is a directory that the return moves into
  the notes of the lease; what an earlier lease left at `notes.local` is set aside into notes of its own.
- The clean rule `{ delete = "notes.local" }` of `.branchdeck.toml` goes: the return takes the notes out itself.
