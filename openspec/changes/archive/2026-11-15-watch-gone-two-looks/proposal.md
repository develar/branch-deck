# Proposal

## Why

The review of c8fbdb965: the watch took its repository as gone at any error of the stat of its git directory, at one
look, so an external disk or a network share away for a moment (`NotFound`, `EIO`, `EACCES`) closed the user's windows.

## What Changes

- The sessions close only when two looks in a row find the git directory missing (`NotFound`); a deleted repository
  is still found within about 2 s.
- Another error of the stat leaves the sessions, is told once in the log, and the next look asks again; a look that
  can't tell compares no file and reads no registry.
