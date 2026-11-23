# Proposal

## Why

A move by the small index (an index file of 16 MB or more) puts its entries back by `update-index --index-info`, which
writes no stat data, and refreshes the copy: the refresh hashes a file by the attributes of the working tree, while
the merge wrote it by those of its index. Under a local edit of a `.gitattributes` (`n.crlf -text` over
`*.crlf text eol=crlf`) the file the merge wrote is not its entry by those attributes, so the refresh left it without
stat data, and every `git status` hashed it and called it modified. On the whole index `read-tree -u` records the stat
data of every file it writes, and git takes the file as unchanged until it changes. The two paths left different
indexes; `the_attributes_of_a_move_are_those_of_the_index_in_both_indexes` saw it only when the whole index was written
in another second than its files (git hashes a racily clean entry again), and failed on the Studio.

## What Changes

- Where a `.gitattributes` of the working tree is not its entry in the merged small index (by its lines: CRLF is the
  same), the put back refreshes the entries the merge wrote once more, on those paths alone, by the attributes of the
  merge: `write-tree --missing-ok` of the small index into its temporary object directory and
  `--attr-source=<tree> add --refresh` of the copy. Otherwise the move runs the processes it ran.
- The tests of the small index compare the status by the stat data the move recorded (the index file newer than every
  file), not by the second the move ran in.

Capabilities: `repository/update-project`.
