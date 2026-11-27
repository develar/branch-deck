# Proposal

## Why

A reviewer asked what becomes of a fix of a review after landing that conflicts with the commits of the review. The
extend stopped at the first fix whose replay conflicted: that fix was only named in a comment, the review never showed
it, and the fixes after it were not even tried, though the record of the body named them as taken.

## What Changes

- A fix whose replay onto the head of the review conflicts goes commit by commit; the commit that conflicts takes the
  files git cannot merge as master has them after the fix (the commit that landed it, paired by the author, the author
  time and the subject), the `<path>~<side>` copies of a file/directory conflict deleted, so the review shows the fix
  whole, with what master changed in those files since the review. A comment names the fix and the files, once.
- A fix whose conflicting commit master has under no identity of its own (an autosquash, a reword) leaves the head
  where it found it and keeps its comment "it does not apply"; the fixes after it apply all the same. As the commit may
  have landed after the last fetch, the target is fetched first then.
- A repeat after a stop leaves out a commit the head has by its identity, so it adds nothing though a later fix changed
  the files it took.
- `brd request-review` names the fixes that took files (`shown.extended.took_files`).

Capabilities: `branch/shown-reviews`.
