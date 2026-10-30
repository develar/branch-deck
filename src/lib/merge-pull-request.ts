// The words of Merge #7 and Auto-Merge #7, the start of a repository that lands by a merge of its pull request on GitHub
// (spec/branch/landing-routes.spec.md#merge): its label by the pull request, and what it does in the hover of every
// surface that offers it (the slot, the menus, the preview, the review)
import type { MergeMethod, PlannedEffect } from "@/lib/bindings"

/** "rebase", "merge commit", "squash": the method as a sentence names it */
export function formatMergeMethod(method: MergeMethod): string {
  switch (method) {
    case "rebase":
      return "rebase"
    case "merge":
      return "merge commit"
    case "squash":
      return "squash"
  }
}

/** "Merge #7", "Auto-Merge #7": GitHub merges it now, or once its checks pass */
export function formatMergeLabel(number: number, auto: boolean): string {
  return auto ? `Auto-Merge #${number}` : `Merge #${number}`
}

/**
 * "GitHub merges #7 of auth into master by rebase"; with the branches it is built on: "… of oauth with the 2 branches it
 * is built on …"; `auto`: "… once its checks pass"
 */
export function describeMerge(subject: string, branchCount: number, number: number, target: string, method: MergeMethod, auto: boolean): string {
  const builtOn = branchCount > 2 ? ` with the ${branchCount - 1} branches it is built on` : branchCount === 2 ? " with the branch it is built on" : ""
  const when = auto ? " once its checks pass" : ""
  return `GitHub merges #${number} of ${subject}${builtOn} into ${target} by ${formatMergeMethod(method)}${when}`
}

/** The merge of a pull request on GitHub a plan or an outcome names: its number, method and target; `auto` once its checks pass */
export interface PlannedMerge {
  readonly number: number
  readonly method: MergeMethod
  readonly target: string
  readonly auto: boolean
}

/**
 * The merge `effects` name (`PlannedEffect::MergePullRequest`, `EnableAutoMerge`, `MergeStack` of a layer of a stack of
 * GitHub); null of a push or a robot
 */
export function mergeOf(effects: readonly PlannedEffect[]): PlannedMerge | null {
  for (const effect of effects) {
    if (effect.kind === "mergePullRequest" || effect.kind === "enableAutoMerge" || effect.kind === "mergeStack") {
      return { number: effect.number, method: effect.method, target: effect.target, auto: effect.kind === "enableAutoMerge" }
    }
  }
  return null
}
