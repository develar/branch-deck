# A disabled button passes on every key but Enter and Space

## Why

The review of 58a5e4f25 (#237): S pressed on the "Start Unknown" slot of a row said nothing. Base UI 1.9 keeps a
disabled button focusable (`focusableWhenDisabled`) and cancels every key on it but Tab (`useFocusableWhenDisabled`),
and the handlers around it skip a cancelled key: the keys of the row (`BranchTableCard`), Escape of a sub-window
(`installEscapeToClose`), the keys of the previews, the reviews and the lists stopped at any disabled button of theirs.

## What Changes

- `patches/@base-ui__react@1.9.0.patch`: a disabled button cancels only Enter and Space, the keys that would press it
  (Space also scrolls the page under a `role="button"`); a disabled field (an input, a textarea, an editable element)
  still takes no typing at all. Both builds.
- Capability `branch/branch-row`: the keys of the row from a disabled button of it.
- Capability `repository/activity`: Escape closes the window from the disabled Clear Log.
