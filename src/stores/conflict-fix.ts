import { useEffect, useMemo } from "react"
import { createStore, useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { undoRegroup } from "@/hooks/useStackActions"
import type { Commit, ConflictFix, FixComponent } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import { getCommitSubject, getSubjectLine, parseSubject } from "@/lib/commit"
import { describeFixStep, fixTitle } from "@/lib/conflict-fix"
import type { SubjectOf } from "@/lib/conflict-fix"
import { getErrorDetails, notifyError } from "@/lib/errors"
import { branchSyncStore, syncBranches, useBranchSync } from "@/stores/branch-sync"
import type { Branch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"

/** The search of the fix of the conflicts of the last sync (spec/branch/conflict-fix.spec.md) */
export interface ConflictFixState {
  /** The sync result the search is for (`fixKey`); null before the first */
  key: string | null
  status: "idle" | "searching" | "done" | "error"
  fix: ConflictFix | null
  error: string | null
  /** A component is being applied */
  applying: boolean
}

export const conflictFixStore = createStore<ConflictFixState>(() => ({ key: null, status: "idle", fix: null, error: null, applying: false }))

/** The repository, the prefix and the inputs of the last sync: a fix found for other ones is stale */
function fixKey(): string | null {
  const { syncedInputs, isSyncing } = branchSyncStore.getState()
  if (isSyncing || syncedInputs === null) {
    return null
  }
  return `${syncedInputs.repositoryPath}\n${syncedInputs.branchPrefix}\n${syncedInputs.fingerprint}`
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
  const result = await invokeCommand(() => commands.findConflictFix({ repositoryPath: syncedInputs!.repositoryPath, branchPrefix: syncedInputs!.branchPrefix }))
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
    useShallow(store => ({ key: store.key, status: store.status, fix: store.fix, error: store.error, applying: store.applying })),
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

/**
 * Applies a component of the fix: one regroup with Undo in the toast and in Activity, then a sync that shows the first
 * branch the fix fills. A fix the repository moved away from is looked for again, without an error.
 */
export async function applyConflictFix(component: FixComponent): Promise<void> {
  const { fix } = conflictFixStore.getState()
  const { syncedInputs } = branchSyncStore.getState()
  if (fix === null || syncedInputs === null) {
    return
  }
  conflictFixStore.setState({ applying: true })
  const result = await invokeCommand(() => commands.applyConflictFix({ repositoryPath: syncedInputs.repositoryPath, fingerprint: fix.fingerprint, plan: component.plan }))
  conflictFixStore.setState({ applying: false })
  if (result.status !== "ok") {
    notifyError("Cannot apply the fix", result.error)
    return
  }
  if (result.data.kind === "stale") {
    void requestConflictFix(true)
    return
  }
  const applied = result.data.result
  const subjectOf = subjectOfCommit()
  const description = component.steps.map(step => describeFixStep(step, subjectOf)).join("; ")
  showUndoToast(fixTitle(component), () => undoRegroup(applied.undoToken, "Cannot undo the fix"), description)
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
  void syncBranches({ autoScroll: false, autoExpand: false })
}
