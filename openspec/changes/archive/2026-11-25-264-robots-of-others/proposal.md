# Proposal

## Why

Branch Deck listed the robots with `owner=me` only (develar/branch-deck#264). A Safe Merge another person started from
the merge request of a branch of the user (a reviewer after the approval), a `/patronus` comment of theirs or a start
in the web UI was missing: the branch showed no running Safe Push and no result, `brd robot` did not report it, and a
push of the branch went on and failed that Safe Push at the end.

## What Changes

- The robots of a repository are the user's and, whoever started them, those of the pushed branches of the user (the
  virtual branches and stack branches the remote has, but one the baseline has whole). Patronus filters a list by one
  source branch: a full list asks the user's robots and one list per pushed branch; an incremental list of at most
  30 minutes asks everyone's robots of its minutes once, kept only when the user's or of a pushed branch, with the whole
  window of a branch pushed since; a list that failed for one branch fails the read.
- A robot someone else started names who (`PatronusRobot::started_by`, the owner of the list against its `me`): the
  details of its chip ("Started by"), the words of a hold and of the refusal of a push, the cancel it replaces, its
  toasts and Activity, `brd robot` (`started_by`).
- It holds and lands the branch as the user's own does, and the same actions stay offered (Cancel, Restart, Push and
  Restart, a start that replaces it): its words name the person. It is never the robot of a lost start or of a
  `/patronus` comment of the user.
- The check of a push lists the robots of the pushed branches that may hold it (its stack and the stack branches of
  it); `brd status` keeps which pushed branches it listed and asks the whole window of a new one once.

Capabilities: `branch/patronus`, `cli/agent-cli`.
