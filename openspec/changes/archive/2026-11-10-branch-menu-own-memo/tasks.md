# Tasks

- [x] 1.1 `useSourceMemo`, the memo of one subscriber of the stores, for the selectors of the branch menu, the menu of an
  island header, the menu of a kept row, the actions of the review and the read of the Safe Push popover; the caches by
  key that their subscribers shared are gone.
- [x] 1.2 The command palette reads the menu of the branch at hand only while it shows.
- [x] 1.3 `useControlRef` and `useHeadingMenu` read their context with `useContext`, so React Compiler compiles them.
- [x] 1.4 E2E: the palette opened on a row whose menu still fades out as the sync of its unstack comes (WebKit too), and
  the menu of the row the palette listed last during a sync.
