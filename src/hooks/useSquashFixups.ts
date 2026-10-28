import { useMemo } from "react"
import { showSquashedToast } from "@/hooks/useSquashCommits"
import type { ListFixups } from "@/lib/autosquash"
import { formatFixupCount, getListFixups } from "@/lib/autosquash"
import type { AutosquashScope } from "@/lib/bindings"
import { runRewriteIntent, witnessed } from "@/lib/rewrite-run"
import { getAutosquashPlan, useAutosquashPlan } from "@/stores/branch-sync"
import { getSelectedRepositoryPath } from "@/stores/repository"

/** The surface of the squash of the fixups of a list (a branch, the unassigned commits): its popover */
export function squashFixupsSurface(list: string): string {
  return `squash-fixups:${list}`
}

export interface SquashFixupsOptions {
  /** The surface that started it and shows its spinner */
  surface?: string | null
}

/**
 * Squashes the fixups of `scope` into their targets, as the rewrite `squashFixups` of the engine (`runRewriteIntent`)
 * named on the table the window shows, in the repository of the press: the squash, the sync, then the toast with an
 * undo. Resolves after the sync, to false, after an error toast or the info toast of a stopped wait, if nothing was
 * squashed.
 */
export async function squashFixups(scope: AutosquashScope, options: SquashFixupsOptions = {}): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const done = await runRewriteIntent(
    {
      label: scope.type === "fixup" ? "Squashing the fixup" : "Squashing the fixups",
      surface: options.surface,
      repositoryPath,
    },
    witnessed(repositoryPath, { rewrite: "squashFixups", scope }),
    "Cannot squash the fixups",
  )
  if (done === null || done.result.rewrite !== "fixupsSquashed") {
    return false
  }
  showSquashedToast(repositoryPath, `Squashed ${formatFixupCount(done.result.answer.squashedCount)}`, done)
  return true
}

/** Squashes one fixup commit into its target, as planned after the last sync. */
export function squashFixupInto(commitId: string): Promise<boolean> {
  if (getAutosquashPlan() === null) {
    return Promise.resolve(false)
  }
  return squashFixups({ type: "fixup", value: commitId })
}

/** The fixups of a branch (null: the unassigned commits), from the plan loaded after the last sync */
export function useListFixups(branch: string | null): ListFixups {
  const plan = useAutosquashPlan()
  return useMemo(() => getListFixups(plan, branch), [plan, branch])
}
