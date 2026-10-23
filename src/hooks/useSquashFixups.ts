import { useMemo } from "react"
import { undoSquash } from "@/hooks/useSquashCommits"
import type { ListFixups } from "@/lib/autosquash"
import { formatFixupCount, getListFixups } from "@/lib/autosquash"
import type { AutosquashScope } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import { runRewrite } from "@/lib/rewrite"
import { getAutosquashPlan, useAutosquashPlan } from "@/stores/branch-sync"
import { branchSubject } from "@/stores/operations"
import { getSelectedProject } from "@/stores/repository"

/** The surface of the squash of the fixups of a list (a branch, the unassigned commits): its popover */
export function squashFixupsSurface(list: string): string {
  return `squash-fixups:${list}`
}

export interface SquashFixupsOptions {
  /** The list whose fixups are squashed (a branch, the unassigned commits): its row is busy meanwhile */
  list?: string
  /** The surface that started it and shows its spinner */
  surface?: string | null
}

/**
 * Squashes the fixups of `scope` into their targets, if HEAD is still `expectedHead`, as one rewrite (`runRewrite`): the
 * squash, the sync, then the toast with an undo. Resolves after the sync, to false, after an error toast, if nothing
 * was squashed.
 */
export async function squashFixups(scope: AutosquashScope, expectedHead: string, options: SquashFixupsOptions = {}): Promise<boolean> {
  const repositoryPath = getSelectedProject()?.path ?? ""
  const branchName = scope.type === "branch" ? scope.value : null
  const list = options.list ?? branchName
  const result = await runRewrite(
    {
      id: scope.type === "fixup" ? "squash-into" : "squash-fixups",
      subject: branchName === null ? `fixups:${scope.type === "fixup" ? scope.value : scope.type}` : branchSubject(branchName),
      objects: list === null ? [] : [list],
      label: scope.type === "fixup" ? "Squashing the fixup" : "Squashing the fixups",
      surface: options.surface,
    },
    "Cannot squash the fixups",
    wait => commands.squashFixupCommits({ repositoryPath, expectedHead, scope }, wait),
    ({ newHead, previousHead, squashedCount }) => ({
      sync: true,
      after: () => showUndoToast(`Squashed ${formatFixupCount(squashedCount)}`, () => undoSquash(repositoryPath, newHead, previousHead, "Cannot undo the squash")),
    }),
  )
  return result !== null
}

/** Squashes one fixup commit into its target, as planned after the last sync. */
export function squashFixupInto(commitId: string): Promise<boolean> {
  const plan = getAutosquashPlan()
  if (plan === null) {
    return Promise.resolve(false)
  }
  return squashFixups({ type: "fixup", value: commitId }, plan.head)
}

/** The fixups of a branch (null: the unassigned commits), from the plan loaded after the last sync */
export function useListFixups(branch: string | null): ListFixups {
  const plan = useAutosquashPlan()
  return useMemo(() => getListFixups(plan, branch), [plan, branch])
}
