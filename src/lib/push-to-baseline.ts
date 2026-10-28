// The words of Push to master, the start of a repository that lands by a push of the copy of a branch into the branch
// of the baseline on its remote (spec/branch/landing-routes.spec.md#push-to-master): its label by the branches it
// lands, and what it does in the hover of every surface that offers it (the slot, the menus, the preview, the review)
import type { MenuItem } from "@/lib/menu"

/**
 * The item of a step that starts the landing of its subject: "Safe Push", "Push to master" of a repository that lands by
 * a push, "Merge #7" of one that lands by a merge of the pull request
 */
export function isStartItem(item: MenuItem): boolean {
  return item.id === "safe-push" || item.id === "push-to-baseline" || item.id === "merge-pull-request"
}

/** "Push to master", "Push to master · 3": the branch with the branches it is built on, which its copy carries */
export function formatPushToBaselineLabel(target: string, branchCount: number): string {
  return branchCount > 1 ? `Push to ${target} · ${branchCount}` : `Push to ${target}`
}

/**
 * "Push the copy of auth into master: it lands without a robot and without the CI of a pull request"; with the
 * branches it is built on: "Push the copy of oauth with the 2 branches it is built on into master: …"
 */
export function describePushToBaseline(subject: string, branchCount: number, target: string): string {
  const builtOn = branchCount > 2 ? ` with the ${branchCount - 1} branches it is built on` : branchCount === 2 ? " with the branch it is built on" : ""
  return `Push the copy of ${subject}${builtOn} into ${target}: it lands without a robot and without the CI of a pull request`
}
