import { useEffect, useMemo } from "react"
import { createStore, useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { undoRegroup } from "@/hooks/useStackActions"
import type { Commit, ConflictFix, FixComponent } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import { getCommitSubject, getSubjectLine, parseSubject } from "@/lib/commit"
import { branchesOfFix, componentFixing, describeFixStep, fixTitle } from "@/lib/conflict-fix"
import type { SubjectOf } from "@/lib/conflict-fix"
import { getErrorDetails } from "@/lib/errors"
import { runRewrite } from "@/lib/rewrite"
import { branchSyncStore, useBranchSync } from "@/stores/branch-sync"
import type { Branch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import type { OperationKey } from "@/stores/operations"
import { getEffectiveBranchPrefix } from "@/stores/repository"

/** The search of the fix of the conflicts of the last sync (spec/branch/conflict-fix.spec.md) */
export interface ConflictFixState {
  /** The sync result the search is for (`fixKey`); null before the first */
  key: string | null
  status: "idle" | "searching" | "done" | "error"
  fix: ConflictFix | null
  error: string | null
}

export const conflictFixStore = createStore<ConflictFixState>(() => ({ key: null, status: "idle", fix: null, error: null }))

/** The surface of Apply Fix: it shows the spinner of the fix it applies */
export const APPLY_FIX_SURFACE = "apply-conflict-fix"

/** The operation of applying `component`: a rewrite of the branches it moves commits of or to */
function conflictFixOperation(component: FixComponent): OperationKey {
  return { id: "apply-conflict-fix", subject: "conflict-fix", objects: branchesOfFix(component), kind: "rewrite" }
}

/** The repository and the inputs of the last sync, its prefix among them: a fix found for other ones is stale */
function fixKey(): string | null {
  const { syncedInputs, isSyncing } = branchSyncStore.getState()
  if (isSyncing || syncedInputs === null) {
    return null
  }
  return `${syncedInputs.repositoryPath}\n${syncedInputs.fingerprint}`
}

// a sync changes what the fix is for: the running search stops, the next conflict view looks again
branchSyncStore.subscribe((state, previous) => {
  if (state.isSyncing && !previous.isSyncing && conflictFixStore.getState().status === "searching") {
    void commands.cancelConflictFix()
    conflictFixStore.setState({ key: null, status: "idle", fix: null, error: null })
  }
})

/** Looks for the fix of the last sync's conflicts unless it is known or being looked for; `force` looks again */
export async function requestConflictFix(force = false): Promise<void> {
  const key = fixKey()
  const state = conflictFixStore.getState()
  if (key === null || (!force && state.key === key && state.status !== "idle")) {
    return
  }
  const { syncedInputs } = branchSyncStore.getState()
  conflictFixStore.setState({ key, status: "searching", fix: null, error: null })
  const result = await invokeCommand(() => commands.findConflictFix({ repositoryPath: syncedInputs!.repositoryPath, branchPrefix: getEffectiveBranchPrefix() }))
  if (conflictFixStore.getState().key !== key) {
    return
  }
  if (result.status !== "ok") {
    conflictFixStore.setState({ status: "error", error: getErrorDetails(result.error) })
  } else if (result.data.search === "canceled") {
    conflictFixStore.setState({ key: null, status: "idle" })
  } else {
    conflictFixStore.setState({ status: "done", fix: result.data })
  }
}

/** The fix search as a conflict view shows it; starts the search when the view is shown */
export function useConflictFix(enabled: boolean): ConflictFixState {
  const state = useStore(
    conflictFixStore,
    useShallow(store => ({ key: store.key, status: store.status, fix: store.fix, error: store.error })),
  )
  const syncedFingerprint = useStore(branchSyncStore, store => (store.isSyncing ? null : (store.syncedInputs?.fingerprint ?? null)))
  useEffect(() => {
    if (enabled && syncedFingerprint !== null) {
      void requestConflictFix()
    }
  }, [enabled, syncedFingerprint])
  return state
}

function subjectsOf(branches: Readonly<Record<string, Branch>>, unassigned: readonly Commit[]): SubjectOf {
  const subjects = new Map<string, string>()
  for (const commit of [...Object.values(branches).flatMap(branch => branch.commits), ...unassigned]) {
    subjects.set(commit.originalHash, parseSubject(getSubjectLine(getCommitSubject(commit))).rest)
  }
  return id => subjects.get(id)
}

/** The subject of a commit ahead of the baseline, without its prefix, as the last sync grouped it */
export function subjectOfCommit(): SubjectOf {
  const { branches, unassignedCommits } = branchSyncStore.getState()
  return subjectsOf(branches, unassignedCommits)
}

/** `subjectOfCommit` for a component: follows the sync */
export function useSubjectOf(): SubjectOf {
  const branches = useBranchSync(state => state.branches)
  const unassigned = useBranchSync(state => state.unassignedCommits)
  return useMemo(() => subjectsOf(branches, unassigned), [branches, unassigned])
}

/** The steps of the found fix of `branch`, for a conflict window that shows them; undefined without one */
export function fixStepsFor(branch: string | undefined): string[] | undefined {
  const { fix, status } = conflictFixStore.getState()
  const component = branch === undefined || status !== "done" || fix === null ? undefined : componentFixing(fix.components, branch)
  if (component === undefined) {
    return undefined
  }
  const subjectOf = subjectOfCommit()
  return component.steps.map(step => describeFixStep(step, subjectOf))
}

/**
 * Applies a component of the fix: one regroup as one rewrite (`runRewrite`), then the sync that shows the first branch
 * the fix fills, then the toast with Undo, also in Activity. A fix the repository moved away from is looked for again,
 * without an error.
 */
export async function applyConflictFix(component: FixComponent): Promise<void> {
  const { fix } = conflictFixStore.getState()
  const { syncedInputs } = branchSyncStore.getState()
  if (fix === null || syncedInputs === null) {
    return
  }
  await runRewrite(
    { ...conflictFixOperation(component), label: "Applying the fix", surface: APPLY_FIX_SURFACE, repositoryPath: syncedInputs.repositoryPath },
    "Cannot apply the fix",
    wait => commands.applyConflictFix({ repositoryPath: syncedInputs.repositoryPath, fingerprint: fix.fingerprint, plan: component.plan }, wait),
    data => {
      if (data.kind === "stale") {
        return { sync: false, after: () => void requestConflictFix(true) }
      }
      const applied = data.result
      const subjectOf = subjectOfCommit()
      const description = component.steps.map(step => describeFixStep(step, subjectOf)).join("; ")
      const first = component.steps[0]
      const moved = new Set(component.steps.flatMap(step => step.commits))
      requestReveal(
        first === undefined
          ? null
          : {
              branchName: first.kind === "restack" ? first.fixes : first.to,
              commitIds: applied.commits.filter(commit => moved.has(commit.oldId)).map(commit => commit.newId),
            },
      )
      conflictFixStore.setState({ key: null, status: "idle", fix: null })
      return {
        sync: true,
        after: () => showUndoToast(fixTitle(component), () => undoRegroup(syncedInputs.repositoryPath, applied.undoToken, "Cannot undo the fix", "Undoing the fix"), description),
      }
    },
  )
}
