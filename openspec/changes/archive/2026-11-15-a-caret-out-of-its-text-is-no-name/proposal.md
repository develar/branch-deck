# A caret out of its text is no name under the pointer

## Why

The WebKit suite on the Studio failed `subject-marks.spec.ts:511` with an uncaught error of the page, "The index is not
in the allowed range", from `Range.setStart` in `textOffset` under `DiffPainter.wordAt`: WebKit's
`caretRangeFromPoint` gave a caret past the end of its text under a pointer that moved over code the review drew anew,
and the boundary at it threw in the handler of the move (the link of the modifier stayed, the page logged an error).
The real app on macOS runs in WKWebView.

## What Changes

- `caretAt` of `diff-painter.ts` takes a caret out of its node (an offset past the length of its text, or past its
  children) as none: no name under the pointer, and the next move asks again.
- Capability `commit/symbol-navigation`.
