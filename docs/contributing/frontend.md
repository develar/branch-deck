# Frontend Rules

**Status:** Active  
**Updated:** 2026-11-11  
**Tags:** [frontend, react, ui, tailwind, shadcn]

## Summary

How the page is written: the theme and its tokens, the components, the state, the React Compiler, the toolchain,
imports and naming, the patched dependencies, and the React and WebKit behaviour that cost sessions.
[AGENTS.md](../../AGENTS.md) ("Code Style") keeps the rules an agent must not miss and links here; the theme itself is
[UI Theme](../design/ui-theme.md), the wording [UX Writing](ux-writing.md).

## Style

- **Colors**: shadcn semantic tokens only (`text-muted-foreground`, `bg-background`, `bg-muted`, `border-border`, `text-primary`,
  `text-destructive`) plus the custom `canvas`, `success`, `warning`, `info` (`text-success`, `bg-warning/10`); the one
  exception is the `window-button-*` palette of the Linux window buttons (IntelliJ's icons, `WindowControls`). **Never** use
  `dark:` variants; the theme is handled by the tokens, which are the IntelliJ Islands values (`src/index.css`).
- **Window and cards** (Islands layering, see [UI Theme](../design/ui-theme.md)):
  - the page canvas is `bg-canvas`; `bg-muted` is a translucent tint (hover, chips, code), not a surface;
  - top-level blocks (tables, collapsible sections, alerts) are `Card` islands, or `CollapsibleCard`; the toolbar
    (`ConfigurationHeader`) lies on the canvas as the title bar ([Title Bar](../../openspec/specs/repository/title-bar/spec.md));
  - never put `bg-muted`/`bg-background` divs on the canvas, and never recolour a top-level `Card` with `bg-muted`;
  - nested panels inside a card use `rounded-lg` (`rounded-xl` at most).
- **Radii** come only from `--radius` (`0.25rem`) in `src/index.css`. Never add radius classes to `src/components/ui/*`
  or hardcode `rounded-[…]`. To change the feel, change the token. Prefer a coherent token-driven scale over copying
  IntelliJ pixel values.
- **Focus**: one token, `focus-visible:bd-focus` (a solid outline of the ring colour), `bd-focus-inset` on a row, a
  header and a field; never a ring, an outline or a shadow of its own under a focus variant, never the ring colour
  (`pnpm lint:arch`, `focus-ring`); the background is the hover and the selection, never the focus
  ([UI Theme](../design/ui-theme.md)).
- **Spacing**: `flex flex-col gap-*` on the parent, not `space-y-*` or child `mt-*`.
- **Paths**: a path that gives way from its start (`…src/main/kotlin`, its last folder kept) is `StartTruncatedText`
  (`src/components/shared/`), never a `dir="rtl"` of its own (`pnpm lint:arch`, `rtl-cut`): the start of a rtl box is
  its right edge, so a box wider than its path (`flex-1`) put a short path at the right, by the counts of a row of the
  Files tree; the component aligns it left, and a path wider than its box still overflows from its start.
- **Prose in a wide pane** is a `ReadingColumn` (one measure, `max-w-measure`, in the middle, `text-md`); a block in
  it sets no width of its own ([UI Theme → Reading column](../design/ui-theme.md)).
- **Font**: InterVariable from `inter-ui` (the full rsms build; `@fontsource-variable/inter` lacks `ss02`/`cv*`
  features), falling back to the system UI font, at 13px (`text-sm`, the IntelliJ UI size). Commit bodies enable `ss04`
  (I/l/1 disambiguation). `font-mono` is JetBrains Mono (`@fontsource-variable/jetbrains-mono`) without ligatures.
- **Icons**: lucide-react components (`<GitBranchIcon />`). Inside `Button`, add `data-icon="inline-start"` (or `inline-end`)
  and no size classes; the button sizes them. Elsewhere use `size-*` classes (e.g. `size-3`).
- **Components**: `Tooltip` for simple text tooltips, `Popover` for rich content, `Kbd` for keyboard shortcuts,
  `showToast({ title, description, action })` from `@/lib/toast-message` for notifications; a toast of an event of
  an object (a robot, a push, a merge, a review) is `showEventToast` of `src/lib/event-toast.ts`, one per event, its
  later states and their actions in place ([One toast per event](ux-writing.md#one-toast-per-event)),
  never a second toast of the same event. Every user-facing error
  goes through `notifyError` in `src/lib/errors.ts` (sticky toast, logged to the Tauri log plugin, recorded in Activity),
  which words it by its code (`describeError`: a held lock names its holder); a store decides by `error.code`
  (`canceled` toasts nothing), never by `error.message`. A read ahead of what the user asks for next (`prefetch_review`
  as a review opens) runs through `readAhead` of `src/lib/command.ts`: its failure, of the command or of a step before
  it, is a warning of the log, never an error and never a rejection of the page, as the read asked for tells its own.
- **Errors of commands**: no command rejects. Every command answers a `Result` in the bindings, the value of a body
  that cannot fail as `Ok` (the `plain` and `now` kinds, `src-tauri/src/api.rs`; the bindings test
  `every_command_of_the_bindings_answers_a_result` holds it), and the bindings answer every failure of its call as
  `{ status: "error", error }` with an `ApiError` (`commandError` of `src/lib/api-error.ts`, which the export of
  `src-tauri/src/bindings_test.rs` writes into the catch of `typedError`): the error the command answered as is, a
  refusal of Tauri itself ("Command not found", an ACL refusal, invalid arguments: a string) and an `Error` of the IPC
  (a fetch of the E2E bridge, the Air shim) as an `internal` error of its words and a warning of the log, "Cannot run
  <command>", as the backend logged nothing of it. So the `status === "error"` branch is
  the one failure path: no `try`/`catch` around `await commands.…`, no `.catch` of one, and a `void` of one is safe
  where its `.then` handles the error; Oxlint refuses a promise neither awaited, caught nor `void`
  (`typescript/no-floating-promises`). A failure of the e2e mock server (no route, a panic) is an error line of the log
  too, on which the test fails whatever the page does with the answer (`tests/e2e/setup/tauri-mocks.ts`).
- **Wording** ([UX Writing](ux-writing.md)): a toast title says what happened ("Pushed feature-ui"),
  never "Success"/"Error"; errors are "Cannot <verb> <object>" with the reason; a toast about an object opens it
  ("Open in Patronus"); Title Case for buttons, menu items and toast actions, sentence case for the rest; "…", "·",
  "canceled", no exclamation marks.
- **Actions** ([UX Writing → Actions](ux-writing.md#actions)): one builder of `MenuItem`s (stable `id`) per
  object, rendered by every surface; the same actions in every form; a key hint only where the key works; disabled
  with a reason, not hidden; one name per action ("Open Pull Request", "Restart Safe Push").
- **React Compiler**: its Rust port `oxc-transform-react` (`react({ compiler })` in `vite.config.ts`, no Babel)
  compiles every component and hook; a function it would skip fails the build (`panicThreshold: "all_errors"`): no
  disable comment for a React rule (`react/rule-suppression`; use `useEffectEvent`), no `try`/`finally` without
  `catch` (use `promise.finally`), no JSX parameter default ([Rendering Performance](../design/rendering-performance.md)).
  `oxc-transform-react` is pinned to the range `@vitejs/plugin-react` declares; `pnpm update-deps` leaves it alone.
- **Rules and intents** ([Rust as the Domain Authority](../design/rust-domain-authority.md)): a rule of the domain is
  a file of `crates/domain-policy/src/` ([Rule Index](code-map.md#rule-index)) that the session derives into the view; the page
  words its answer (`src/presenters/`: a presenter takes the part its caller selected and reads no store, importing a
  store module for its types only), sends what
  the user asks as an intent (`planIntent`, `runIntent`, `executePlan`, `executeRewrite`, `cancelIntent` of
  `src/model/intents.ts`, the one module that calls `execute` and `cancel_operation`; a command of the app,
  `app(…)` of `crates/app-api/src/commands.rs`, from its one module of `APP_COMMAND_CALLERS`), tells what it shows
  (`src/model/demand.ts`), and reads the key of the selected project before the first `await` of a run, so the run
  acts on the repository of its press. `pnpm lint:arch` enforces it (`rule-module`, `intent-call`, `presenter-state`,
  `late-project-read`), `the_writes_of_the_engine_are_its_intents_and_their_cancel` (`crates/app-api/src/registry_test.rs`)
  keeps every write of a repository an intent of `execute`.
- **Rows of another session**: a store that holds something of a row by the branch's name (a confirm, a popover, a form,
  a dialog, a choice of the user) forgets it under the view of another session of the repository (`onRowsForgotten` of
  `src/stores/view-rows.ts`, a slot of a lease pool leased again), as a row of the same name there is another branch;
  `pnpm lint:arch` (`row-store`) refuses a store that names `branchName` without it, a store that only projects the view
  being an `ALLOWLIST` entry that says so.
- **State**: zustand vanilla stores (`createStore`) read with `useStore` selectors; select narrow slices: the compiler
  memoizes renders, not selectors, which run on every store change. A persisted setting is a `createPersistentStore`
  (`src/stores/settings/persistent-store.ts`): it starts from the snapshot the backend injects and changes only through
  the one writer in Rust (`set_setting` → `SettingsService` of `crates/app-settings/`), applying every window's changes
  by version; no window and no Rust code writes `settings.json` beside it, and a rule on a setting (a default, the
  branch prefix) is Rust's ([Distributed Settings](../design/distributed-settings.md)). Debounce user inputs where
  appropriate (`p-debounce`). An export of `src/` that no page code calls is deleted with the tests of it alone; the
  seams of the unit tests stay, which no page code calls by design: the `reset…` of a module, its state between tests
  (`resetPatronus`, `resetPullRequests`, `resetReviewReadiness`, `resetReviewRequests`, `resetRobotDetails`,
  `resetDemand`, `resetDock`, `resetEventToasts`, `resetFailingAnnouncements`), and the waits `whenRepositoryViewCurrent`
  and `whenSessionReady` (a test waits for the session or the view it answered).
- **Base UI**: compose with the `render` prop (`<TooltipTrigger render={<Button />} />`), not `asChild`. Base UI emits
  `data-open`/`data-closed`; set `data-state` manually where E2E tests read it.
- **shadcn workflow**: components are generated into `src/components/ui/` (`components.json`, style `base-rhea`). Run
  the CLI with `npx shadcn@latest …` (`add <component>`, `docs`, `init --preset`), then adapt the files.
  `pnpm dlx shadcn@latest` crashes with `o.deepPartial is not a function`: `@shadcn/registry` uses the zod 3 API and
  pnpm dlx resolves zod 4.
- **Locally customised ui files**: `alert`, `badge`, `button`, `card`, `chart`, `checkbox`, `combobox`,
  `context-menu`, `dropdown-menu`, `input`, `input-group`, `popover`, `resizable`, `select`, `spinner`, `switch`,
  `tabs`, `textarea`, `toggle`, `toggle-group`, `tooltip`, `toast` in
  `src/components/ui/` carry a `// local customisation: …` header naming the edits. After `add --overwrite` (e.g. a
  style switch), re-apply them; `git diff HEAD -- src/components/ui` shows what was lost.
- **Toolchain** (Vite+, `vite-plus`, the `vp` CLI): one pinned toolchain of Vite 8, Vitest, Oxlint and Oxfmt,
  configured in `vite.config.ts` (`lint`, `fmt`); Tailwind CSS v4 through `@tailwindcss/vite`, its theme tokens in
  `src/index.css`. Oxlint is type-aware through tsgolint (TypeScript 7, the Go compiler, strict, checked with `tsc` on
  `tsconfig.app.json`, `tsconfig.node.json` and `tsconfig.e2e.json`) and runs the React Compiler rules; there is no ESLint, because typescript-eslint cannot use
  TypeScript 7. Oxfmt formats code and config (not Markdown) and sorts imports and Tailwind classes. Test files import
  from `vite-plus/test`.
- **TypeScript**: Use `u32` for timestamps and `snake_case` for properties that bind to Rust structs.

## Imports and Naming

- Import every React API, component, hook, store, utility and type explicitly; types with `import type`.
- Project modules only through the `@/` alias (`@/stores/branch-sync`), never `../`. shadcn: `import { Button } from "@/components/ui/button"`.
- Generated Rust bindings: `import { commands } from "@/lib/bindings"` (written by Specta from `src-tauri/src/lib.rs`).
- Heavy or rarely shown components: `React.lazy` with the named export, rendered inside `Suspense`.
- **Components**: PascalCase `.tsx` files with a named export (`BranchTableCard.tsx`). **Hooks**: camelCase with the
  `use` prefix in `src/hooks` (`useRowSelection.ts`). **Stores**: `src/stores/*.ts` (`branch-sync.ts`), persisted
  settings in `src/stores/settings/`. **Rust-bound props**: `snake_case`, as in the generated bindings.

## Patched Dependencies

A bug of a dependency that the page cannot get around in its own code is fixed by a pnpm patch,
`patches/<name>@<version>.patch`, named in `patchedDependencies` of `pnpm-workspace.yaml`. Its header, above the
changes, says the bug, the fix, the upstream code it stands for, how to port it and its tests; pnpm reads the changes
and ignores the header. The lockfile holds the hash of each patch: a checkout runs `pnpm install` after it changes.

- **Make or change one**: `pnpm patch <name>@<version> --edit-dir notes.local/<dir>`, edit both builds (`.mjs` and `.js`),
  `pnpm patch-commit notes.local/<dir>`; then put the header back above the changes (pnpm writes none), drop what pnpm
  adds that is not yours (a deleted `.npmignore`), and run `pnpm install` for the hash.
- **An upgrade fails loudly**: `allowUnusedPatches` and `allowNonAppliedPatches` stay unset, so `pnpm install` stops with
  "The following patches were not used" while a patch is keyed to a version the lockfile no longer has, and with "error
  applying hunk" when a hunk no longer fits. Port the patch onto the new version by its header (a new key and file),
  run its tests, or drop it once a release fixes the bug.
- **Now**: `@base-ui/react@1.9.0`, a popup that went took the open delay of its trigger's hover, a flushSync of a toast
  in the delivery of its observers, and a disabled button that cancelled every key ([Pitfalls](#pitfalls)).

## Pitfalls

- **An effect event runs only in an effect or its callback**: a `useEffectEvent` function called in a `setState`
  updater runs in the render, as React applies updaters there, and throws (#440, every form of a branch name failed
  so); decide in the callback and pass the value into the updater (`useDebouncedRead`).
- **React Compiler and store reads**: a function that reads stores with `getState()`, called in the render body beside
  `useStore`, is memoized on its arguments and shows a stale value. The value must come out of a selector, or be an
  argument the hook selected: `useStepInputs` beside `useRowPart(name, "landing")` (`src/hooks/useBranchNextStep.ts`);
  a menu builder likewise (`useBranchMenu`, `src/hooks/useBranchContextActions.ts`).
- **A part of a row answers the whole part**: `useRowPart(name, "landing")` answers the next step of the subject, a new
  object with every patch of it, each object in it new too (until 2026-11-17 the next step of every branch was patched
  twice after each sync that moved a tip: it waited for the landing needs of the new tips, then had them; now the needs
  of the tips before stand meanwhile, but a patch of the status of a row is one of the whole part). A component that shows
  one field of a part selects it: a primitive by itself (`useRowPart(name, "landing", step => step?.node.ring ??
  false)`), an object kept while it says the same (`useRowPart(name, "landing", step => step?.status, sameJson)`),
  never an object `select` builds without `sameJson` (see the next pitfall). Read off the whole part, the ring of the
  node made a sync of one more commit on one branch render 131 of the 141 rows of `perf:table`, and a status read off
  it each status cell (`part-field-read` of `pnpm lint:arch` catches only a field read inline off the answer; the
  render counter, `row-renders.spec.ts`, is the guard).
- **A selector answers one object per state, of its own**: `useSyncExternalStore` reads a selector again after each
  commit and renders again for another object, so a selector that builds (a menu, the read of a popover) keeps what it
  built in a memo of its subscriber (`useSourceMemo`), keyed by the stores and by every argument. A cache by a key that
  two subscribers share, whose other arguments differ, answers each a new object on every read: the menu of a row that
  still faded out and the palette opened on that row rendered each other again until React gave up (#185, a blank
  window).
- **A hook that calls only `use(…)` is not compiled**: React Compiler compiles a function named `use…` only when it
  calls a hook named `use` and a capital (`useContext`, `useState`); `use(Context)` alone leaves it plain, and a
  callback it returns is new on every render (`useControlRef` gave every trigger a new ref: React detached and attached
  it, a menu item left its menu and came back, with each render). Read a context in such a hook with `useContext`.
- **The effects of a kept view run again**: a view of the main window (React's `Activity`, `KeepMounted` in
  `IndexPage`) destroys the effects of its components as it hides and runs them again as it shows, state and refs kept.
  An effect that acts once (a replay, a focus) marks it in a ref, as `useLazyRoot` does; a popup root closes as its view
  hides (`useCloseWhenViewHides`). WebKit, the engine of the app, keeps the focus on a hidden element until a later
  layout: test the focus and the popups in `pnpm e2e:webkit` too (`{ tag: "@webkit" }`).
- **Content that moves under a still pointer**: WebKit tells the over and the enter of the element that came under it
  at once (trusted, at the same position), and moves the pointer where it is after a scroll. A hover opens a popup only
  after a move over its trigger (`hoverOpensOnMove`, `src/lib/pointer-moves.ts`); another element keeps it only as
  the same control, never by its place: the same root of `src/components/ui`, or the `controlKey` (action and subject)
  of a component that renders its root anew. Better keep the element: one tree in every state of a control, its
  Stop Waiting a sibling in a wrapper that is there without it too; a new element loses the focus as well, and the
  keys with it. Its E2E test brings a trigger under the pointer with `bringUnderStillPointer` and runs the delay with
  `page.clock.fastForward`. The other way round, Chromium and WebKit tell the trigger that moved from under a still
  pointer that the pointer left it: an open submenu stays open until the next move (`hoverClosesOnMove`), which gives the
  trigger the leave as a move, and Base UI closes the submenu only for a move away from it. The list of an open menu
  never moves (`frozenMenuGroups` in `MenuItems`: an item that leaves stays disabled in its place, one that comes shows
  at the next open, the openings counted by the menu roots, `useMenuOpenings`), so only a scroll of the menu moves a
  trigger from under the pointer there; an item that takes the place of another shares its `slot`.
- **What comes late beside a hovered control goes where it moves nothing**: a control that comes beside another goes on
  the side away from the edge of its place that stands (`fixedEdge`): the left in the header and in the step track of a
  row, which stand at their end (Stop Waiting of Update, Stop Waiting and the actions of a lost start of Safe Push), the
  right in the robot card, whose line starts at its left, and the details of its robot, which the session reads as it
  opens, past its actions, away from the edge the card hangs by (`data-hang` of the popup: its chip's edge, or the bottom
  of a card beside its chip shifted up against the bottom of the window; why a failed robot failed, read ahead, stands
  before them, where it stood as the card opened), and its first row of actions next to that edge, so the second row
  that goes moves none; on the screen a card hung by its bottom edge reads bottom up while the focus follows the DOM,
  actions first: accepted. A popover over a moving anchor holds still while the pointer is in it (the details of a chip
  of Status, `useHeldAnchor` of `StatusChip`: their chip moves as the Actions of its row widen), pinned where it stood,
  hung by the edge it hung by, without collision avoidance, so what comes or goes changes its other edge only (what
  grows may take it past the edge of the window while the pointer is in it: accepted), and stands by the anchor again
  once the pointer left; what stands above the actions of a robot card says only what the card knew as it opened (the
  start line of a running robot, a profile known then), what its details tell later stands past them; a control with nothing to show keeps its place (the
  landing indicator, empty and disabled while nothing is on the way; the time of the last sync, empty before the first
  sync); the items of Status and Actions stand at the end
  of their group, also where the head of the column is wider than they are; a lock wait of a form or a popover
  (`LockWaitNotice`) comes on the line of its buttons before them, or below them, never on a line of its own above them,
  and in a dialog over the lines of its footer, which keep their room (`LinesUnderLockWait`), its spinner in the place
  of the icon of the button; the first push of a repository on no service comes with its row, its facts answered from
  git; the alert of a failed sync stays while the next sync runs (`syncFailure`), as "Last synced" does. Each case has
  an E2E test: the pressed control keeps its place, its popup open (`repository-lock.spec.ts`, `update-project.spec.ts`,
  `header.spec.ts`, `late-facts.spec.ts`, `patronus-actions.spec.ts`, `column-widths.spec.ts`, `onboarding.spec.ts`)
- **A wait the user started is heard and reached by key**: beside the Stop Waiting of a surface that started a rewrite
  (`LockWaitNotice`, `StopWaitingButton`) a polite live region, there before the wait, says whom it waits for without
  how long and without the backticks of the command line, which a screen reader reads aloud
  (`presentLockWaitAnnouncement`), so once per holder; the visible notice, whose time grows every second, is hidden from
  it. Stop Waiting takes the focus as the wait shows, where the press left it on the pressed button, in its
  form, dialog or popover, or nowhere, and gives it back as the wait ends, else to the button that waited, before it
  leaves the document (a layout effect: a popover closes on a focus that fell to the page); the pressed
  button stays focusable while it is busy (`focusableWhenDisabled`, `aria-busy`). `Button` dims a disabled button kept
  focusable (`focusableWhenDisabled`) unless it is busy, its spinner says that, so no caller dims by hand; and no control
  that is `aria-disabled="true"` takes a hover or a press, its own nor its caller's (the `hover` and `active` variants of
  `src/index.css`, whose `:where` leaves the specificity as it was). Tested in
  `repository-lock.spec.ts`, by the pointer and by key, and in `amend-changes.spec.ts` for the progress row of a form,
  which takes the place of the pressed button, in both engines
- **The words of a tooltip are the description of its trigger**: Base UI links none, and a closed tooltip has no
  content in the page, so the root renders the words of its `TooltipContent` in a hidden span at the end of the page and
  `TooltipTrigger` names it in `aria-describedby`: a screen reader hears the reason of a disabled control. The words
  are a string: the content, or beside content that is not one a `description` the type asks for (a short phrase by
  [UX Writing](ux-writing.md); `Shortcut: D` beside a name and a key), never a copy of the content. Not beside the
  trigger, whose parent's text it would join. A lazy root describes its trigger once it mounted and adds no node
  before. A trigger that wraps the control the focus reaches (the span of `MenuItemButton`) gives it the description
  too (`TooltipDescribedChild`); `description={false}` for a tooltip that only says the name of its trigger or its
  start ("Dismiss" of "Dismiss the amend conflict"); a key goes as `Shortcut: <key>`. The `TooltipContent` is a direct
  child of its `Tooltip` (or in a fragment or a list of its children), where the root finds its words: one that a
  component of its own renders gives the trigger no description. Tested by
  `toHaveAccessibleDescription` (`facts-without-answer.spec.ts`, `review-requests.spec.ts`,
  `foreign-operation.spec.ts`)
- **A popup that goes took the hover of its trigger** (Base UI 1.9): each popup part shares the timers of its trigger's
  hover (`useHoverInteractionSharedState`) and cleared them as it unmounted at the end of its exit. The pointer back over
  the trigger of a popup closed otherwise than by its hover (its view hid, a press of an item) started a delay that the
  popup took with it as it went, and the popup opened only at the next move; a hidden view keeps a closed popup until
  it shows, a loaded machine ends an exit late. `patches/@base-ui__react@1.9.0.patch` leaves the timers to the trigger
  ([Patched Dependencies](#patched-dependencies)); the tests of "Hover back over a trigger whose popup closed"
  (`branch-row-layout.spec.ts`) fail without it.
- **A disabled button cancelled every key** (Base UI 1.9): `useFocusableWhenDisabled` keeps a disabled button focusable
  and cancelled every key on it but Tab, and a handler around it skips a key someone took (`event.defaultPrevented`):
  the keys of a branch row stopped at its disabled Safe Push or push, Escape of a sub-window at a disabled button.
  `patches/@base-ui__react@1.9.0.patch` cancels only Enter and Space on a disabled button, every key but Tab on a
  disabled field; a handler around a button checks `defaultPrevented` as before. Tested in `foreign-operation.spec.ts`,
  `repository-lock.spec.ts` and `activity.spec.ts`, in both engines.
- **A flushSync in the delivery of a ResizeObserver** renders every update pending then, which can resize other observed
  elements after their reports, and WebKit renders a commit of React that waited in a microtask after the callback in
  that same rendering update: a ResizeObserver loop. Toast.Content of Base UI 1.9 set the height of its toast so; the
  same patch sets it in the next frame (an issue for upstream is drafted). Every observer of ours is made by
  `observeSize` (src/lib/observe-size.ts, `pnpm lint:arch` `resize-observer`). The update delivers by depth: after a
  report only what is deeper than the elements it reported. The page is watched from a report of an observer (ours or a
  library's) to the end of its delivery, the next task or the next frame. A change in a callback or in the chain of
  microtasks after it (`SPAN_MICROTASKS`, 256: the flushSync that places a popup of floating-ui comes 42 to 87
  microtasks after the callback of its observer) can resize any element, as the layout is the page's (a block above a
  view that grows shrinks the view, a sibling that grows shrinks it in a flex row, a view hidden around a table empties
  its cells), so every element of ours no deeper than the elements of the latest report leaves its observers and is
  observed anew as the delivery ends (its size a frame later, as the loop would give it); a change past the chain (a
  render of React in a task, a microtask of the update later still) lets go the elements it is in, so the new rows of a
  sync let go none of the cells of the others. A deeper one stays, the update delivers it in that frame; one let go comes back at once when a report of
  shallower elements shows it deeper. An element first observed in the delivery, no deeper than the report (a block a
  flushSync mounted, observed by its ref; a table mounted again), is observed only as it ends. A change after the
  delivery lets nothing go.
  The observers of the libraries go through the engine's own class with one step before their callback, which opens
  the delivery at the depth of their report as one of ours does (`watchTheLibraries`): the same class, prototype,
  methods and `instanceof`, the same entries in the same frame, none of their elements let go. An element let go so in
  10 of 20 frames (`CYCLE_RELEASES`, `CYCLE_FRAMES`: every frame, every other) after its report there, resized by a
  change of a callback of ours (its own observer's or another's), is a cycle, logged as an error: frames, not time, so a window that renders slowly tells it
  too, and a drag that resizes it between frames is none, nor one let go before its report (a commit lets an element go once, however many of our observers observe it: a
  diff view has three); its release is measured against its layout as reported, one arithmetic for both
  (`sameBoxOf` of `src/lib/laid-out.ts`), as the computed style lays it out otherwise than the engine at a zoom of the
  app other than 100% or a fractional scale of WebKit. A cycle that resizes its element after the delivery (in a frame callback, a timer) is not
  caught: it resizes the element once a frame, as an animation, a transition, a drag or a live resize of the window do
  for as long as they last, and no size, frame or time tells it from them. The page logs a loop of a library's
  observers as a warning (one may come under load) and 3 within 5 s as an error (`LOOP_REPEATS`, `LOOP_REPEAT_MS`); a
  loop that left an element of ours pending is an error at once, so every E2E test fails on one, its line naming each
  element by its test id or slot, else by its tag and role in the nearest ancestor with one (`div[treeitem] in
  review-sidebar`). Its elements are told
  by the engine's own boxes: the computed style proposes the candidates, laid out otherwise than reported (exact at a
  zoom of 100%, up to 1.5 px off at another in WebKit), and an observer made first of all observes them at the loop,
  its report in the next update the verdict, by which the page weighs the loop at its own time. A candidate that
  changed or left the page before that update (a render of a task, a frame callback, a transition, a remount) is told
  nothing of, so a pending one that changed again is no error, rarer and safer than a false one. The probe observes
  the root too, to report in that update even when no candidate has a box: its report lowers the depth that update
  takes its next iteration from to the root's, so a library's loop in every frame comes in every other while candidates
  wait (still 3 within 5 s), and `observeSize` lets go more than the update skips, which costs a report. The page cancels the
  engine's event of a loop, so no console reports it as uncaught; each kind of line
  comes once in 10 s at most, with the count it left out (`LOOP_LOG_MS`; `src/lib/page-errors.ts`, the thresholds of
  `src/lib/resize-loops.ts`). The WebKit check `tests/e2e/resize-loops/` names the elements of a library's loop in its
  report. `react-resizable-panels` 4.18 makes no flushSync in its observers.
- **A disabled root does not hear the pointer that came**: Base UI listens on the trigger of a disabled tooltip only once
  it is enabled, so a tooltip that got its words with the pointer resting on it (the description of Approve) opened
  only at the next move. The tooltip replays the hover of its trigger as it is enabled (`hoverAgain`), for a pointer
  that moved over it, never for content under a still pointer.
- **A context menu by a key**: the engine's own `contextmenu` of the context menu key or Shift+F10 goes to the element
  with the focus, at the middle of it or at the selection: it opens a menu only when the focus is inside the trigger,
  and never at the place the menu is about. A view that knows the place takes the key (`isMenuKey`) and opens the menu
  itself with `openMenuByKey` (`src/lib/menu-key.ts`: the rows of pull requests, the code of a diff), never by a
  `contextmenu` dispatched by hand: Windows opens the engine's menu of the context menu key as the key goes up, at
  whatever has the focus by then, and only the helper holds that release back. The menu takes the focus a frame after
  it shows: an E2E test waits for `toBeFocused()` of the menu before its arrows.
- **WebKit clears the selection as the focus leaves it**: the focus going to an element that does not hold the
  selection (a menu, a list, a field) leaves the page with no selection at all, where Chromium keeps it. A view whose
  keys read the selection keeps its ends and puts them back as the focus returns (`keptSelection` of `DiffPainter`); a
  test of "the name is still selected after the popup" is a `@webkit` test.
