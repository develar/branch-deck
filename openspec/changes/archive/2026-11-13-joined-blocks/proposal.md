# Proposal

## Why

A check on a real Kotlin stack found a pure reformat (indent 4 to 2, a lambda body put on its own line) that left two
changes to read in each function of this shape, while the review plan of the same diff folded all its blocks into the
unit of whitespace:

```
-    fun formatted(): String {
-        return entries.joinToString("\n") { formatEntry(it) }
+  fun formatted(): String {
+    return entries.joinToString("\n") {
+      formatEntry(it)
     }
+  }
```

The old `    }` that ends the function is the new `    }` that ends the lambda: the comparison of lines anchors on it and
cuts the one change into two blocks. The first lacks a `}` on its new side, the second adds one, so neither is the same
tokens alone, and the marks, which read each block by itself, proved neither. The plan compares a whole declaration
(`units::whitespace_pair`), so it and the reading disagreed about one diff.

## What Changes

- The first reading of the blocks reads a row of blocks that keep a start, with the lines between them, as one region
  (`diff_marks::marks_joined`): asked by its characters without blanks first, then read as one block by its tokens,
  and taken when the parts of its runs, cut at the ends of the blocks, leave no line of them to read. A part may have
  lines on one side alone.
- Bounded by work: at most eight blocks a region, the tokens of a region within `MAX_BLOCK_TOKENS`, its comparison
  within the work of the file.
- The diffs and the merges in the stores are of another model (`MODEL_VERSION` 20, `MERGE_MODEL_VERSION` 13), so a
  stored diff with a start in each such block is read again, and a kept reading with it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: blocks the comparison cut out of one change are read as one; a run may have lines on one
  side alone as the part of such a run.

## Impact

- `crates/diff-marks/src/{marks,marks_joined,marks_runs}.rs`, `crates/file-diffs/src/{service,merge}.rs`, the docs of
  `LineRun`; `docs/design/diff-view.md`.
