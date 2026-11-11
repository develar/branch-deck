# Proposal

## Why

The requirement "The menu of the code acts on the name it opened on" said that a key opens the menu at the caret and
that the menu then acts on the name at the selection. No key opened it: the engine sends the `contextmenu` of the
context menu key and of Shift+F10 to the element with the focus, the view, and the trigger of the menu is inside the
view; the event came at the middle of the view, not at the caret. What the fallback did in fact was another thing: a
right click beside every name acted on a name selected elsewhere, in another row, and no test covered it.

Making the key real showed three more defects of the same menu:

- the caret of a point beside the text is the nearest place of the text, so the pointer on the number of a line or in
  the space before its first word was "on" that word: the menu of the code offered its declaration, and the modifier
  underlined it;
- Windows opens the engine's menu of the context menu key as the key goes up, at whatever has the focus by then, whether
  its press was prevented or not (checked in Edge, the engine of WebView2): a menu of the app opened on the press, as
  the rows of pull requests open theirs, got a second menu on the release;
- WebKit clears the selection of the page as the focus goes into the menu or a list: after the popup closed nothing was
  selected, and ⌘B, ⌥F7 and the next menu key read no name.

## What Changes

- The pointer opens the menu of the code on the name under it and on none where it is on no name; a name selected
  elsewhere plays no part.
- The context menu key and Shift+F10 open the menu of the code on the name at the selection, under the start of that
  name (`openCodeMenuByKey`, `codeMenuKeyPoint`), while the view itself has the focus.
- `openMenuByKey` (`src/lib/menu-key.ts`) is the one way a key opens a menu of the app: it prevents the press, the next
  release of the context menu key, and opens once for a key held down. The rows of pull requests use it.
- `wordAt` gives a word only when the point is on its text.
- The painter keeps the ends of the selection of the view and puts them back as the focus returns to the view.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/symbol-navigation`: the menu of the code by the pointer and by a key, the name under the pointer, the
  selection across a popup of the view.
- `repository/review-requests`: the context menu key of a row opens one menu.

## Impact

- `src/lib/menu-key.ts` (new), `src/lib/review-row-keys.ts`, `src/lib/diff/code-menu.ts`,
  `src/components/diff/DiffView.tsx`, `src/components/diff/diff-painter.ts`.
- `tests/e2e/commit-files/symbol-navigation.spec.ts`, `src/lib/menu-key.test.ts`, `src/lib/diff/code-menu.test.ts`.
- `docs/design/diff-view.md`, `docs/contributing/frontend.md`.
