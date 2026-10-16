# Rendering Performance

**Status:** Implemented  
**Updated:** 2026-10-16  
**Tags:** [frontend, react, zustand, performance]

## Problem

The main window shows every branch of a repository, with about 20 store subscriptions per row: the branch, its tip
and remote tip, the robot, the pull request, the push state, the landing span. A row re-renders its whole subtree
(actions, chips, the Safe Push slot) when one of them changes. Without automatic memoization, a component that skips
work does so only through a hand-written `memo` or `useCallback`, and one inline prop is enough to undo it.

## Solution Overview

[React Compiler](https://react.dev/learn/react-compiler) compiles every component and hook of `src/`, so a subtree
re-renders only when its inputs change, also without a hand-written `memo`. The build refuses code the compiler would
skip, so no component falls back to running unmemoized without anyone noticing.

## Design

### Key Decisions

1. **Decision**: React Compiler through Babel (`babel-plugin-react-compiler` in `@rolldown/plugin-babel`, the
   `reactCompilerPreset` of `@vitejs/plugin-react`, `vite.config.ts`), not the Rust port (`compiler: true`).
   **Rationale**: the Babel plugin is the stable 1.0 release; the Rust port (`oxc-transform-react`) is experimental.

2. **Decision**: `panicThreshold: "all_errors"`: a function the compiler cannot compile fails `vite build` and the dev
   server's transform.
   **Rationale**: by default the compiler skips such a function quietly and it runs without memoization. Its usual
   causes are caught early instead:
   - an `eslint-disable` of a `react-hooks` rule: read the latest values through `useEffectEvent` instead
     (`InlineForm`, `CommitReviewCode`);
   - `try` with `finally` but no `catch`: use `promise.finally(…)` (`SquashCommitsDialog`);
   - a JSX element as a parameter default: use `prop ?? <… />` in the body (`ui/toast.tsx`).

3. **Decision**: the existing `memo`, `useMemo` and `useCallback` stay.
   **Rationale**: the compiler keeps them (`preserve-manual-memoization`), and they document which identities the
   code relies on, for example the stable `getId` of `useRowSelection`.

4. **Decision**: the compiler does not replace narrow store selectors.
   **Rationale**: `useStore` runs on `useSyncExternalStore`, which calls every subscriber's selector on every
   `setState`. Memoization saves the render, not the selector. Selectors stay narrow (a primitive, a joined string, or
   an interned record), as before.

### Checking what the compiler compiled

The `logger` option of `reactCompilerPreset` reports one event per function: `CompileSuccess`, or `CompileError` with
the reason. A temporary copy of `vite.config.ts` with
`logger: { logEvent(file, event) { if (event.kind !== "CompileSuccess") console.log(file, event) } }` lists
everything the compiler skipped. With `all_errors`, a `CompileError` stops the build anyway.

## References

- `vite.config.ts`: the plugin and its options
- `eslint.config.mjs`: `eslint-plugin-react-hooks` `recommended`, which includes the compiler rules
- [Distributed Settings](distributed-settings.md): the persisted stores the selectors read
