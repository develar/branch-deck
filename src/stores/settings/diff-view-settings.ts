import { z } from "zod"
import { useStore } from "zustand"
import type { WhitespacePolicy } from "@/lib/diff/file-diff-model"
import { createPersistentStore } from "./persistent-store"

/*
 * One choice for every diff view, as IntelliJ's diff settings: the layout (`DiffViewToggle`, Settings → Diffs & Merge),
 * soft wraps and ligatures. A default only fills a missing field, so a stored value wins over it.
 */
/** The narrowest share of a side of Split */
export const MIN_SPLIT_RATIO = 0.15

const DiffViewSettingsSchema = z.object({
  // Split first, as IntelliJ's diff and its VCS log; Inline is Unified with a lightly edited line as one row
  mode: z.enum(["split", "unified", "inline"]).default("split"),
  // off, as IntelliJ's USE_SOFT_WRAPS: a long line keeps its shape and scrolls
  softWraps: z.boolean().default(false),
  // on, unlike IntelliJ's "Enable ligatures": a ligature of JetBrains Mono keeps the width of its cells
  // (docs/design/ui-theme.md, "Diffs")
  ligatures: z.boolean().default(true),
  // the share of the old side in Split, as IntelliJ's DiffSplitter: its divider drags, a double click makes it 0.5
  splitRatio: z
    .number()
    .min(MIN_SPLIT_RATIO)
    .max(1 - MIN_SPLIT_RATIO)
    .catch(0.5)
    .default(0.5),
  // what the diff engine ignores; IntelliJ's default compares every character
  whitespacePolicy: z.enum(["default", "trim", "ignore", "ignoreBlankLines"] satisfies WhitespacePolicy[]).default("default"),
})

type DiffViewSettings = z.infer<typeof DiffViewSettingsSchema>
export type DiffViewMode = DiffViewSettings["mode"]

const diffViewStore = createPersistentStore("diffView", { schema: DiffViewSettingsSchema, isArray: false, isMainOnly: false })

/** The layout of every diff view */
export function useDiffViewMode(): DiffViewMode {
  return useStore(diffViewStore, state => state.mode)
}

/** Long lines break at the edge of the diff instead of scrolling */
export function useDiffSoftWraps(): boolean {
  return useStore(diffViewStore, state => state.softWraps)
}

/** The share of the old side in Split, in every diff view */
export function useDiffSplitRatio(): number {
  return useStore(diffViewStore, state => state.splitRatio)
}

export function setDiffSplitRatio(splitRatio: number): void {
  diffViewStore.setState({ splitRatio: Math.min(Math.max(splitRatio, MIN_SPLIT_RATIO), 1 - MIN_SPLIT_RATIO) })
}

/** The ligatures of JetBrains Mono in the code of every diff view */
export function useDiffLigatures(): boolean {
  return useStore(diffViewStore, state => state.ligatures)
}

export function setDiffLigatures(ligatures: boolean): void {
  diffViewStore.setState({ ligatures })
}

/** What the diff engine ignores in every diff view */
export function useDiffWhitespacePolicy(): WhitespacePolicy {
  return useStore(diffViewStore, state => state.whitespacePolicy)
}

/** The policy outside a component: the reads a window starts with its page */
export function getDiffWhitespacePolicy(): WhitespacePolicy {
  return diffViewStore.getState().whitespacePolicy
}

export function setDiffWhitespacePolicy(whitespacePolicy: WhitespacePolicy): void {
  diffViewStore.setState({ whitespacePolicy })
}

export function setDiffViewMode(mode: DiffViewMode): void {
  diffViewStore.setState({ mode })
}

export function setDiffSoftWraps(softWraps: boolean): void {
  diffViewStore.setState({ softWraps })
}
