---
name: bd update
description: Requirements for bd update, the agent's Update Project, its undo, and the resolve session in which an agent resolves a conflicting update commit by commit outside the working tree.
style: plain-1
targets:
  - ../../crates/bd-update/src/update_command.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/domain-types/src/error.rs
  - ../../crates/bd-cli/src/schema.rs
  - ../../crates/update-core/src/update_project.rs
  - ../../crates/update-core/src/resolve_session.rs
  - ../../crates/update-core/src/resolve_copies.rs
  - ../../crates/sync-utils/src/paths.rs
  - ../../crates/git-ops/src/update_resolve.rs
  - ../../crates/git-ops/src/resolve_check.rs
  - ../../crates/git-ops/src/merge_tree_output.rs
  - ../../crates/git-ops/src/commit_writer.rs
  - ../../crates/git-ops/src/update_branch.rs
  - ../../crates/git-ops/src/revs.rs
---

# bd update

Status: Implemented
Date: 2026-10-15

## Summary

`bd update` is the app's Update Project for a coding agent ([Update Project](../repository/update-project.spec.md)):
it fetches the baseline and brings the checked out branch onto it. A conflict starts a resolve session. The session
replays the local commits outside the working tree and opens one round per conflicting commit. The agent edits copies
of the conflicting files, and `bd update --continue` commits each round. When nothing conflicts any more, the finish
moves the branch, and the files follow through the guarded update. All state of a session is in git; only the copies
that the agent edits are files. How it works: "Resolve session" in [Git Operations](../../docs/design/git-operations.md).
The rest of the CLI: [Agent CLI](agent-cli.spec.md).

## Goals

- An agent updates and resolves conflicts in a working tree that other sessions share: no stash, no rebase in the
  working tree, no checkout.
- Peers may commit while the agent resolves: their commits become more rounds, not a failure.
- Every resolution is explicit and checked, and the user sees what each resolution dropped.
- A crash or a refusal at any step leaves a state that the next command completes or removes.

## Non-goals

- Binary, file/directory, rename, mode, symlink and submodule conflicts: a round refuses them.
- A partial `--continue`: a round needs a resolution for every conflicting file.
- Skipping a commit that the baseline likely has by its author, author time and subject only: bd marks it. Only a
  patch id that equals one of the baseline proves that the baseline has the change.
- Notes that follow a later rewrite of a resolved commit.

## Requirements

### Update

- `bd update` must do what the app's Update Project does: fetch the baseline, check the update, then, under the
  repository lock, check that nothing moved and apply a clean update (`up_to_date`, `fast_forward`, `rebase`), the
  files following through the guarded working-tree update. It must print the state, the previous and the new HEAD, the `replayed` and
  `dropped_already_upstream` counts, the landed commits (`landed`) and the undo token
  `update:<previous head>..<new head>` with full ids.
  [@test] ../../crates/bd-update/src/update_test.rs (`update_fast_forwards_and_the_undo_moves_the_files_back`, `update_rebases_the_local_commits_and_keeps_unrelated_local_changes`)
- The fetch must run before the lock and through the remote command policy; a failed fetch is `network_error`
  (exit 5, retryable). A held lock is `locked` (exit 3) after the fetch.
  [@test] ../../crates/bd-update/src/update_test.rs (`a_failed_fetch_is_a_network_error`, `the_update_waits_for_the_repository_lock_after_its_fetch`)
- `--no-fetch` must update onto the last fetched baseline. `--dry-run` must imply `--no-fetch`, take no lock and
  print what the update would do, writing at most git objects. A dry run of a conflict must print `state: "conflict"`
  with the likely conflicting files, the local commits and the upstream commits that change them, start no session,
  change nothing, and exit with 1. The output goes into an agent's context: `files` lists the first 50, the ones
  with their commits, and `files_omitted` counts the rest; never one left out: 51 are all listed, with their commits.
  [@test] ../../crates/bd-update/src/update_test.rs (`dry_run_neither_fetches_nor_writes`, `a_dry_run_reports_the_files_and_commits_of_a_conflict_and_changes_nothing`, `a_dry_run_of_a_wide_conflict_lists_the_first_files_and_counts_the_rest`)
- With `branchdeck.resolveByWords` on (the default), an update whose every conflict of git resolves by words must
  apply as a clean `rebase`, with no session. The commit that the words resolve keeps the author and the message of
  the local commit. The output must list each one in `resolved`: the `commit`, its `new_id` (omitted for a dry run),
  `conflicts`, `file_count` and `files` with their conflicts. Each one must get a note in
  `refs/notes/branch-deck/resolve` with `resolved-by-words` and a `words` line per file.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`an_update_whose_conflict_resolves_by_words_applies_and_names_the_commit`)
  [@test] ../../crates/git-ops/src/update_resolve_test.rs (`an_update_whose_every_conflict_resolves_by_words_applies_without_a_session`)
  [@test] ../../crates/git-ops/src/word_replay_test.rs (`a_conflict_that_resolves_by_words_is_committed_and_the_rest_replays_onto_it`, `a_commit_whose_words_conflict_too_stops_the_replay`)
- A rebase, merge, cherry-pick or revert in progress must be `operation_in_progress` (exit 4) for the update and its
  dry run, moving neither the branch nor the files.
  [@test] ../../crates/bd-update/src/update_test.rs (`update_and_its_dry_run_are_refused_during_a_cherry_pick`)
- A `git switch` between the check and the move must move no branch: `head_moved` for the update, `undo_stale` for
  the undo.
  [@test] ../../crates/bd-update/src/update_test.rs (`a_switch_between_the_check_and_the_move_moves_no_branch`, `a_switch_during_the_move_fails_on_the_head_lock`)
- A lock file of git in the way of the move (`HEAD.lock` or the lock file of the branch), which a killed git can leave,
  must be `locked` (exit 3, retryable) with the path of the file in the message, and move nothing.
  [@test] ../../crates/bd-update/src/update_test.rs (`a_stale_head_lock_is_locked_and_names_the_file`)
- A `bd update` or `bd undo` killed between the move of the branch and its files must leave the mark
  `refs/branch-deck/update-pending/<worktree id>`, which the move creates in its ref transaction. The next `bd update`,
  `bd undo` or Update Project of the app must let the files follow first. When the branch moved since, the result is
  `finish_pending` (exit 4), changing nothing, with the command that removes the mark. `bd update --status` must print
  the mark as `pending_update`, with `next`.
  [@test] ../../crates/bd-update/src/update_test.rs (`an_update_killed_before_its_files_followed_is_finished_by_the_next_update`, `an_undo_killed_before_its_files_followed_is_finished_by_the_next_undo`, `an_interrupted_update_of_a_branch_that_moved_since_is_reported`)
- With `core.autocrlf=true`, the files of an update and of its undo must get the line ends of a checkout, and the
  status must stay clean.
  [@test] ../../crates/bd-update/src/update_test.rs (`an_update_with_autocrlf_checks_out_crlf_and_leaves_a_clean_status`)
- Uncommitted, staged or untracked files on paths the update writes must be `uncommitted_overlap` (exit 4, not
  retryable) with those `paths`, also for a dry run; bd must never stash, reset, check out or commit them. `paths`
  lists the first 100 (all 101 rather than one left out); with more, the message says "`paths` lists the first 100 of
  <n>: `git status` lists all".
  [@test] ../../crates/bd-update/src/update_test.rs (`uncommitted_changes_on_updated_files_block_with_their_paths`, `many_blocking_paths_list_the_first_100_and_say_how_many`)
- `bd undo update:<previous head>..<new head>` must take only full ids and move the branch back with the files
  following, under the lock; it must refuse with `uncommitted_overlap` when local changes overlap the paths it writes,
  `undo_stale` (exit 4, not retryable) when the branch moved or HEAD switched since, and `invalid_arguments` when the
  token is not the last update of the branch. It must print whether the landed report is the one from before the
  update again (`landed_restored`). An undo of an undone update must exit 0 with `already_undone: true`. The finish of
  a resolve session prints the same token, and the same undo takes it back.
  [@test] ../../crates/bd-update/src/update_test.rs (`the_undo_is_refused_when_local_changes_overlap_or_the_branch_moved`, `update_outputs_match_the_schema`, `update_fast_forwards_and_the_undo_moves_the_files_back`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`undo_needs_the_token_of_the_last_update`, `undo_tokens_are_strict`)
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)

### Resolve session

- A conflict of `bd update` must start a resolve session under the same lock, and change no ref but the session ref
  and no file but the copies: HEAD, the index and the working tree stay. The output is `state: "conflict"` with a
  `session` object, and the exit code is 1.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)
- The session must belong to one worktree and one branch, and keep the baseline it started with. Its ref must be the
  shared `refs/branch-deck/resolve/<worktree id>` (`main`, or the name of a linked worktree), so that a
  `gc --prune=now` in another worktree keeps its objects. In another worktree, `--continue` must be `no_session`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_session_belongs_to_its_worktree_and_survives_a_gc_in_another`)
- The first conflicting commit must be found with a binary search over the prefixes of the local commits, at most
  ⌈log₂ n⌉ replays after the check; after a resolution, one replay of the rest, and a search only when it conflicts.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`the_first_conflict_is_found_with_a_binary_search`)
- Every commit the session writes must have the committer of the session start (name, email and date), so the same
  input gives the same ids; `--continue --dry-run` must report the `new_id` that `--continue` writes.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_dry_run_of_continue_checks_and_gives_the_ids_of_the_real_one`)
- `bd update` with a session open must fetch nothing and change nothing: it prints the open round again and writes
  its missing copies. With no round open (the finish is next), it prints `state: "pending"` (exit 1) and
  `session.next` `<bd> update --continue`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`, `local_changes_on_the_finish_keep_the_session_ready`)
- Commits that peers make on top of the session's HEAD must join the session as more local commits, and a conflict
  among them opens a round. A HEAD that no longer contains the session's HEAD, or that left the branch, must be
  `session_stale` (exit 4, not retryable); the session stays for `--abort`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`peers_commits_on_top_become_more_rounds`, `a_rewritten_head_makes_the_session_stale_and_abort_removes_it`)
- A history that git can't read (an object that is not there) must be an error with git's message, never
  `session_stale`: only exit 1 of `merge-base --is-ancestor` is "no".
  [@test] ../../crates/git-ops/src/update_resolve_test.rs (`a_session_head_git_cannot_read_is_an_error_not_a_rewrite`)

### Rounds

- A round must print `session` with the `worktree`, the `state` (`resolving`, `ready`, `applying`, `applied`), the
  `round`, `replayed` and `total`, the `dir` of the copies, the `commit` (`commit`, `subject`, `body`, `author`, the
  absolute path of its `patch`), the conflicting `files`, `marker_labels` (the full ids after
  the markers: `onto`, `before` as `<commit>^`, and `commit`), and `next`, the command that continues. `next` names
  this binary in this repository, as the app's prompt does (`<bd> -C <repo> update --continue --round <n>`; `bd` when
  the binary is unknown).
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`, `session_outputs_match_the_schema`, `markers_are_found_by_their_labels`)
- A round must not repeat the rounds resolved before it: each output of a long session would grow with all of them in
  the agent's context. `session.resolutions` (the rounds so far) is printed by `bd update --status` only; the finish
  prints all in `resolutions`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_second_conflict_opens_a_second_round_with_its_own_copies`, `local_changes_on_the_finish_keep_the_session_ready`)
- A round must print `session.blocked_paths`: the uncommitted, staged or untracked files on paths the update likely
  writes (the paths the upstream changed since the merge base that differ between HEAD and the upstream, with the
  check of every sync), the first 100 (101 rather than one left out), and `blocked_paths_omitted` the count of the rest. They block the finish, so
  the agent asks the user about them first. The finish runs the exact check.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`local_changes_that_will_block_the_finish_are_named_from_the_start`)
- Each file must have its `path`, its `kind` (`content`, `modify_delete`), the absolute paths of `edit` (the merge
  result with zdiff3 markers), `before` (the commit's parent), `onto` (the new parent: the baseline and the commits
  replayed so far) and `commit` (the commit's version), a side left out when it has no such file, `markers` (the
  blocks left in `edit` now), a `hint`, and the newest 10 `upstream_commits` that changed it since the merge base.
  On Windows every path of a copy must have backslashes, and the `\\?\` form only from 260 characters on, which
  Windows PowerShell 5.1 opens without long paths enabled.
  [@test] ../../crates/sync-utils/src/paths_test.rs (`a_windows_path_has_backslashes_and_the_verbatim_form_only_from_260_characters`)
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`, `modify_delete_is_resolved_with_take_and_delete`)
- A local commit whose patch id (`patch-id --stable`) equals that of a baseline commit since the merge base must be
  left out of the replay, as `git rebase` leaves it out, and listed as `landed_dropped` with that baseline commit (in
  each round and at the finish); it counts in `dropped_already_upstream`. The commits around it replay as runs, so a
  conflict after it still opens its round.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_commit_the_upstream_has_with_the_same_patch_is_left_out`, `a_conflict_after_a_left_out_commit_opens_its_round`)
- A commit that the baseline likely has by its author, author time and subject only must get the hint "likely
  landed", never be skipped: that key is no proof.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`likely_landed_commits_get_the_hint`)
- Only text conflicts (`CONFLICT (contents)`, an add on both sides as a `content` conflict without `before`),
  modify/delete and rename/delete (kind `rename_delete`, with `renamed_from`; `--take` and `--delete` take either path)
  must be supported. A mode that one side changed must be merged as git does; two modes that differ on
  both sides are `distinct modes`. Any other type (binary, file/directory, rename, distinct modes, a symlink or a
  submodule, an invalid path), and a `content` file whose merge result has no block of bd's labels, must refuse the
  whole round with `unsupported_conflict` (exit 4) and its `paths`; at the start no session is created.
  The same rename on both sides merges cleanly and opens no round.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`binary_and_file_directory_conflicts_are_refused_and_start_no_session`, `a_rename_and_a_delete_resolve_with_a_take`)
  [@test] ../../crates/git-ops/src/update_resolve_test.rs (`only_text_and_modify_delete_conflicts_are_supported`)
  [@test] ../../crates/git-ops/src/merge_tree_output.rs (`parses_the_tree_the_stages_and_the_messages`, `a_clean_merge_has_only_the_tree`)
- With `branchdeck.resolveByWords` on, a conflicting commit whose conflict of git its words resolve must be a round
  that resolves itself: it counts in `round`, and the agent gets the next round that does not resolve. Every round
  and `--status` must list these rounds in `session.resolved` (`round`, `commit`, `new_id`, `conflicts`, `files`), so
  a round number never skips without a reason. Their resolution has `words` (the conflicts), and each file `how:
  "words"` with its `conflicts`. A commit that the words leave without a change is dropped, as `git replay` drops one.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_round_that_resolves_by_words_is_named_in_the_next_round_the_status_and_the_finish`)
  [@test] ../../crates/git-ops/src/update_resolve_test.rs (`a_round_that_its_words_resolve_counts_and_the_agent_gets_the_next_one`, `a_later_round_that_its_words_resolve_resolves_in_the_continue_and_the_finish_notes_it`)
- A second conflicting commit must open a new round with its own directory; the copies of the closed round are
  removed. `--round <n>` of another round must be `round_moved` (exit 3, retryable: read the open round and retry): its message says that
  `bd update` prints the open round.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_second_conflict_opens_a_second_round_with_its_own_copies`)

### Copies

- The copies must be in `branch-deck/resolve/r<n>/` of the worktree's git dir: `edit/`, `before/`, `onto/` and
  `commit/`, each file under the flat name `<key>-<basename>`, and `commit.patch`. The tree paths must be validated: no
  empty, `.` or `..` component, no `.git` in any case or a name the file system takes for it (`GIT~1`, `.git.`, HFS+
  ignorable code points, as `core.protectNTFS` and `core.protectHFS`), no NUL; on Windows also no name Windows can't
  hold (`\`, a drive prefix, a device name such as `CON`, `<>:"|?*` or a control character, a trailing dot or space),
  which a tree made on Linux or macOS may have. Two paths that differ only in case get distinct names. A
  basename longer than 100 bytes is cut on a character boundary, and keeps its extension.
  [@test] ../../crates/git-ops/src/update_resolve_test.rs (`tree_paths_that_could_leave_a_directory_are_invalid`, `copies_have_flat_names_that_never_collide`, `a_long_basename_is_cut_and_keeps_its_extension`)
- bd must write and read the bytes as git stores them, without filters, through a new temporary file and a rename,
  and never through a symlink: a symlink at `edit` is no resolution, and a rewrite replaces it. Every directory from
  the git dir (or `--dir`) down to the copy must be a real directory: bd neither reads nor deletes through a symlinked
  directory.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_symlink_at_edit_is_never_followed`, `the_dir_of_another_worktree_or_session_is_refused_and_a_symlinked_round_is_never_read`)
- The marker file `.branch-deck-resolve` must name the git dir of the worktree that owns the directory. bd deletes
  only in a directory whose marker names its own git dir.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`the_dir_of_another_worktree_or_session_is_refused_and_a_symlinked_round_is_never_read`)
- `--dir <path>` must put the copies of a new session into an absolute directory that is not a symlink, not inside a
  worktree, empty or marked as bd's for this git dir, and used by no other session; a refusal is
  `invalid_arguments`, and a refused directory that bd created is removed. bd must delete only what it wrote, and
  remove only a directory it created.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`the_copies_go_to_dir_only_outside_every_worktree`, `the_dir_of_another_worktree_or_session_is_refused_and_a_symlinked_round_is_never_read`)
- `bd update --status --materialize` must write the missing copies of the open round, and with `--force` all of them.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`markers_an_edit_outside_a_block_and_a_missing_file_are_refused`)

### `--continue`

- `bd update --continue` must check the resolution of every conflicting file of the open round, commit the round,
  replay the rest, and open the next round (exit 1) or apply the update (exit 0). A refusal must change nothing, name
  the round and its commit (id and subject, also in `error.commits`), and say that the round stays open and the edit
  copies keep the agent's edits; `--status` then shows `edited` per file, whether its copy differs from bd's merge
  result.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_second_conflict_opens_a_second_round_with_its_own_copies`, `markers_an_edit_outside_a_block_and_a_missing_file_are_refused`, `a_refusal_names_the_commit_and_keeps_the_round`)
- A resolution must be an edited `edit` copy, `--take onto:<path>`, `--take commit:<path>` or `--delete <path>`; a
  missing file is never a delete. No resolution is `missing_resolution`; an unchanged modify/delete copy is no
  resolution either. A take of a side without the file deletes it. A `--take` or `--delete` of a path the round does
  not have, or a side other than `onto` and `commit`, is `invalid_arguments`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`modify_delete_is_resolved_with_take_and_delete`)
- A block must be recognised only by the labels bd passed to the merge (`<<<<<<< <onto>`, `||||||| <commit>^`,
  `>>>>>>> <commit>`), at any marker length (`conflict-marker-size`), with the separator of the same length only
  between the base section and the end. With a rename, merge-ort adds `:<path>` after a label, and the block counts
  too: a renamed file with a conflict resolves as any other. Any other marker-like line, e.g. the `=======` of a
  Markdown setext heading, is text.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`markers_are_found_by_their_labels`, `a_renamed_file_with_a_conflict_resolves_with_an_edit`)
  [@test] ../../crates/git-ops/src/resolve_check_test.rs (`a_setext_heading_is_text`, `longer_markers_are_found_by_their_labels`, `markers_of_a_renamed_file_carry_the_path`)
- An edited content file must equal bd's `edit` copy outside the conflict blocks (`resolution_outside_conflict`), and
  have no marker line of bd's labels where a block was (`unresolved_markers`). When only the line ends differ (CRLF and
  LF, or the newline at the end), the message says so. A copy that starts with a byte order mark or is UTF-16, which
  `Set-Content` and `Out-File` of Windows PowerShell 5.1 write, is `resolution_outside_conflict` with its own message:
  write the copy as UTF-8 without a BOM, with `[IO.File]::WriteAllText`. All three refusals are exit 2 with `paths`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`markers_an_edit_outside_a_block_and_a_missing_file_are_refused`, `an_edit_copy_with_a_byte_order_mark_or_in_utf16_is_refused`)
  [@test] ../../crates/git-ops/src/resolve_check_test.rs (`a_resolution_inside_the_block_passes`, `an_edit_outside_the_block_is_refused`, `markers_left_are_refused`, `the_plain_runs_between_two_blocks_must_stay_in_order`, `changed_line_ends_are_named`)
- The message of a refusal must name the absolute `edit` copy of each path. For leftover markers and a missing
  resolution, it must also name the paths that changed in the working tree, where bd never reads. The refusal of a
  change outside the blocks must say that such a change goes into a `fixup! <subject>` commit after the update, then
  `bd fold`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`an_edit_in_the_working_tree_is_pointed_to_the_copy`)
- The lines of a side's change (`before` to `onto`, `before` to `commit`) that the result lost must be reported per
  file as `dropped_upstream_lines` and `dropped_commit_lines`, also for `--take` and `--delete`, never refused. A line
  counts as kept when the result has it as it is, or when one line that replaced a conflict block (of an edit; the
  whole result of a take) has every word that the side added to its nearest `before` line (the largest share of words
  in common). So a lost change of one value (a mode, a version, a flag) is reported, also when its word is elsewhere
  in the file, and a rewrite that keeps the change is not.
  [@test] ../../crates/git-ops/src/resolve_check_test.rs (`dropped_lines_are_the_lines_of_a_side_that_the_result_lost`, `a_lost_change_of_one_value_is_reported`, `a_lost_value_is_reported_when_its_word_is_elsewhere_in_the_file`)
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_take_reports_the_lines_of_the_other_side_it_drops`, `field_shaped_conflicts_resolve_as_the_hints_say`)
- The hint of a modify/delete whose commit deletes the file must say how to keep the delete (`--take commit:<path>`)
  and that a change of `onto` that belongs in another file now goes into a `fixup!` commit after the update, reported
  as `dropped_upstream_lines`. The same line added on both sides merges cleanly and opens no round.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`field_shaped_conflicts_resolve_as_the_hints_say`)
- A resolution that leaves the commit without changes must be `empty_resolution` (exit 2), nothing written.
  `--allow-empty` keeps it as an empty commit (`empty: "kept"`); `--drop-empty <commit>` (the round's commit, at
  least 4 characters of its id) drops it (`empty: "dropped"`, `new_id` null, no note). The finish counts a dropped
  commit as `dropped_by_resolution`, never in `dropped_already_upstream` or `landed`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_resolution_without_changes_is_refused_unless_the_agent_keeps_or_drops_it`)
- `--continue --dry-run` must run the checks, report the resolution with its `new_id`, exit 0, and move no ref. It
  must take no repository lock, so a check in a loop never makes an update or a rewrite wait; only the real
  `--continue` is `locked` while another process holds the lock.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_dry_run_of_continue_checks_and_gives_the_ids_of_the_real_one`, `a_dry_run_of_continue_answers_while_another_process_holds_the_lock`)

### Finish

- The finish must move the branch from the session's HEAD to the replayed tip with the reflog message
  `branch-deck: update project`, the files following through the guarded update, and print `state: "rebase"`, the
  `undo_token` and `resolutions`: per round the `commit`, its `new_id` and per file `how` (`edited`, `onto`,
  `commit`, `deleted`, `words`) with the dropped lines. The rounds that resolved themselves are in `resolved` too. The
  session ref and the copies must be gone afterwards.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)
- Each resolved commit must get a note in `refs/notes/branch-deck/resolve` (the session, the round, the original
  commit, `resolved-by-words` for a round that resolved itself, the outcome per path), written only by the finish, in
  its ref transaction.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)
- Local changes on paths the finish writes must be `uncommitted_overlap` (exit 4) with the `paths`, and keep the
  session `ready` with its resolutions; a later `--continue` only finishes.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`local_changes_on_the_finish_keep_the_session_ready`)
- A finish interrupted before the branch moved (`applying`), or refused by a `git switch` (`head_moved`), must be
  completed by the next `--continue`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`an_interrupted_finish_is_completed_by_the_next_continue`)
- A finish whose branch moved but whose files did not follow (`applied`, HEAD at the tip) must refuse `--abort` with
  `finish_pending` (exit 4), and the next `--continue` must let the files follow.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_finish_killed_before_the_files_followed_is_recovered`)
- When the files did follow and the branch still contains the tip (the finish died before it deleted the session
  ref, also with peers' commits on top), both `--continue` and `--abort` must delete the session ref and exit 0.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_session_whose_finish_died_after_the_files_followed_ends_after_a_peer_commit`)
- When the files did not follow and HEAD left the tip, `--continue` and `--abort` must be `finish_pending` (exit 4),
  and the message must name `git update-ref -d <session ref> <commit>` as the way out.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_moved_branch_without_its_files_is_finish_pending`)

### `--abort` and `--status`

- `bd update --abort` must delete the session ref with compare-and-swap and the copies bd wrote, under the lock, and
  print the `worktree`, the `branch` and the `round`; it writes no note. Without a session it is `no_session` (exit 2).
  `--worktree <id>` must abort the session of another worktree, e.g. an orphaned one. While that worktree exists, only
  `--force` aborts it. An id must be letters, digits, `.`, `_` and `-`, else `invalid_arguments`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_rewritten_head_makes_the_session_stale_and_abort_removes_it`, `an_orphaned_session_is_listed_and_aborted_by_its_worktree`, `aborting_the_live_session_of_another_worktree_needs_force`)
- Temporary refs older than an hour (by the time in their name) belong to a process that is gone; the start of a
  session, `--abort` and a `--continue` of an `applying` or `applied` session must delete them.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`stale_temporary_refs_are_swept_when_a_session_starts`)
- `bd update --status` must take no lock and change nothing: it prints the session of this worktree as a round prints
  it, and `sessions`, every session of the repository with its `worktree`, `branch`, `git_dir`, `state`, `round`,
  `orphaned` (its worktree is gone) and `own`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`a_session_belongs_to_its_worktree_and_survives_a_gc_in_another`, `an_orphaned_session_is_listed_and_aborted_by_its_worktree`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)
- `bd schema` must have `update`, `update_status` and `update_abort`, and the real outputs must use only the
  properties they declare.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`session_outputs_match_the_schema`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)

### Error codes

| Exit | Codes |
|---|---|
| 1 | a round is open (`conflict`) or the finish is next (`pending`): an output, not an error |
| 2 | `no_session`, `unresolved_markers`, `resolution_outside_conflict`, `missing_resolution`, `empty_resolution`, `invalid_arguments` |
| 3 | `head_moved`, `round_moved`, `locked` (retryable) |
| 4 | `session_stale`, `unsupported_conflict`, `uncommitted_overlap`, `finish_pending` |

- The refusals of a resolution and `unsupported_conflict` must list the files in `paths`.
  [@test] ../../crates/bd-update/src/resolve_test.rs (`markers_an_edit_outside_a_block_and_a_missing_file_are_refused`, `binary_and_file_directory_conflicts_are_refused_and_start_no_session`)

### `bd help update`

- It must print the session workflow: the round object with `blocked_paths`, `marker_labels` and `next`, `pending`,
  edit only the files at `edit` (a change outside the blocks waits for a `fixup!`), `--continue` with `--take`,
  `--delete`, `--allow-empty` and `--drop-empty`, the exit codes, the report of the resolutions, the build after exit 0
  and a `fixup!` with `bd fold` or `bd undo` on a failure, `finish_pending`, `--abort` (with `--force` for a live
  session of another worktree), `--status` and `--dir`, and that a copy is written as UTF-8 without a BOM
  (`[IO.File]::WriteAllText` in Windows PowerShell). `bd help agent` must only name it; together the two texts must name every
  option of `bd update`.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

## Testing / Local Run

- `cargo nextest run -p bd-cli resolve_test` runs the session end to end on `TestRepo` fixtures; some use a `BD_GIT`
  wrapper that counts the replays, fails a step once or runs a `git switch` at a chosen moment.
- `cargo nextest run -p git-ops resolve_check update_resolve merge_tree_output` covers the checks, the conflict types
  and the parser.

## Open Questions / Risks

- An unsupported conflict in a later round stops the session: `--abort` drops the earlier rounds too.
- `session.blocked_paths` is the cheaper check of every sync; the exact check runs only at the finish.
- A note does not follow a later amend or reword of its commit: it is an audit record of one session.
- `--continue --dry-run` writes the resolved commit on a temporary ref and deletes it: no ref differs afterwards.

## References

- [Git Operations](../../docs/design/git-operations.md): "Resolve session" and "Update project".
- [Update Project](../repository/update-project.spec.md): the app's update and the copied prompts.
- [Agent CLI guide](../../docs/guidelines/agent-cli.md).
