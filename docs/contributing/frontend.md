# Frontend Rules

**Status:** Active  
**Updated:** 2026-10-29  
**Tags:** [frontend, react, ui, tailwind, shadcn]

## Summary

How the page is written: the theme and its tokens, the components, the state, the React Compiler, the toolchain,
imports and naming, and the React and WebKit behaviour that cost sessions. [AGENTS.md](../../AGENTS.md)
("Code Style") keeps the rules an agent must not miss and links here; the theme itself is
[UI Theme](../design/ui-theme.md), the wording [UX Writing](ux-writing.md).

## Style

- **Colors**: shadcn semantic tokens only (`text-muted-foreground`, `bg-background`, `bg-muted`, `border-border`, `text-primary`,
  `text-destructive`) plus the custom `canvas`, `success`, `warning`, `info` (`text-success`, `bg-warning/10`); the one
  exception is the `window-button-*` palette of the Linux window buttons (IntelliJ's icons, `WindowControls`). **Never** use
  `dark:` variants; the theme is handled by the tokens, which are the IntelliJ Islands values (`src/index.css`).
- **Window and cards** (Islands layering, see [UI Theme](../design/ui-theme.md)):
  - the page canvas is `bg-canvas`; `bg-muted` is a translucent tint (hover, chips, code), not a surface;
  - top-level blocks (tables, collapsible sections, alerts) are `Card` islands, or `CollapsibleCard`; the toolbar
    (`ConfigurationHeader`) lies on the canvas as the title bar ([Title Bar](../../spec/repository/title-bar.spec.md));
  - never put `bg-muted`/`bg-background` divs on the canvas, and never recolour a top-level `Card` with `bg-muted`;
  - nested panels inside a card use `rounded-lg` (`rounded-xl` at most).
- **Radii** come only from `--radius` (`0.25rem`) in `src/index.css`. Never add radius classes to `src/components/ui/*`
  or hardcode `rounded-[…]`. To change the feel, change the token. Prefer a coherent token-driven scale over copying
  IntelliJ pixel values.
- **Spacing**: `flex flex-col gap-*` on the parent, not `space-y-*` or child `mt-*`.
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
  (`canceled` toasts nothing), never by `error.message`.
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
- **Locally customised ui files**: `alert`, `badge`, `button`, `card`, `chart`, `checkbox`, `context-menu`,
  `dropdown-menu`, `input`, `popover`, `select`, `spinner`, `tabs`, `textarea`, `tooltip`, `toast` in
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

## Pitfalls

- **React Compiler and store reads**: a function that reads stores with `getState()`, called in the render body beside
  `useStore`, is memoized on its arguments and shows a stale value. The value must come out of a selector, or be an
  argument the hook selected: `useStepInputs` beside `useRowPart(name, "landing")` (`src/hooks/useBranchNextStep.ts`);
  a menu builder likewise (`useBranchMenu`, `src/hooks/useBranchContextActions.ts`).
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
  `page.clock.fastForward`.
