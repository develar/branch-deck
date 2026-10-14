import { useMemo } from "react"
import type { ListFixups } from "@/lib/autosquash"
import { formatFixupCount, getListFixups } from "@/lib/autosquash"
import type { AutosquashScope } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { undoSquash } from "@/hooks/useSquashCommits"
import { getAutosquashPlan, syncBranches, useAutosquashPlan } from "@/stores/branch-sync"
import { getSelectedProject } from "@/stores/repository"

/**
 * Squashes the fixups of `scope` into their targets, if HEAD is still `expectedHead`, then syncs and offers an undo.
 * Resolves (before the sync ends) to false, after an error toast, if nothing was squashed.
 */
export async function squashFixups(scope: AutosquashScope, expectedHead: string): Promise<boolean> {
  const repositoryPath = getSelectedProject()?.path ?? ""
  const result = await invokeCommand(() => commands.squashFixupCommits({ repositoryPath, expectedHead, scope }))
  if (result.status !== "ok") {
    notifyError("Failed to Squash Fixups", result.error)
    return false
  }

  const { newHead, previousHead, squashedCount } = result.data
  showUndoToast(`Squashed ${formatFixupCount(squashedCount)}`, () => undoSquash(repositoryPath, newHead, previousHead))
  void syncBranches({ autoScroll: false, autoExpand: false })
  return true
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
