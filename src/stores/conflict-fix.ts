import { useEffect, useMemo } from "react"
import { createStore, useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { undoRegroup } from "@/hooks/useStackActions"
import type { Commit, ConflictFix, FixComponent, MoveCommitsResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { getShownSubject } from "@/lib/commit"
import { componentFixing, describeFixStep, fixTitle } from "@/lib/conflict-fix"
import type { SubjectOf } from "@/lib/conflict-fix"
import { getErrorDetails } from "@/lib/errors"
import { runRewriteIntent, witnessed } from "@/lib/rewrite-run"
import { showUndoToast } from "@/lib/undo-toast"
import { branchSyncStore, useBranchSync } from "@/stores/branch-sync"
import type { Branch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"

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

/**
 * The repository and the commits of the table the session left (`commitsRevision`): a fix found for other commits is
 * stale; a push or the deletion of an archived branch leaves it as it is
 */
function fixKey(): string | null {
  const { syncedRepositoryPath, commitsRevision, isSyncing } = branchSyncStore.getState()
  if (isSyncing || syncedRepositoryPath === null) {
    return null
  }
  return `${syncedRepositoryPath}\n${commitsRevision}`
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
  const repositoryPath = branchSyncStore.getState().syncedRepositoryPath!
  conflictFixStore.setState({ key, status: "searching", fix: null, error: null })
  const result = await invokeCommand(() => commands.findConflictFix({ repositoryPath }))
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
  const syncedCommits = useStore(branchSyncStore, store => (store.isSyncing || store.syncedRepositoryPath === null ? null : store.commitsRevision))
  useEffect(() => {
    if (enabled && syncedCommits !== null) {
      void requestConflictFix()
    }
  }, [enabled, syncedCommits])
  return state
}

function subjectsOf(branches: Readonly<Record<string, Branch>>, unassigned: readonly Commit[]): SubjectOf {
  const subjects = new Map<string, string>()
  for (const commit of [...Object.values(branches).flatMap(branch => branch.commits), ...unassigned]) {
    subjects.set(commit.originalHash, getShownSubject(commit))
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
 * Applies a component of the fix: one regroup as the rewrite `applyConflictFix` of the engine (`runRewriteIntent`) in
 * the repository of the press, a rewrite of the branches it moves commits of or to; once it is written, before the sync
 * shows it, the first branch the fix fills is revealed and the fix is forgotten; then the toast with Undo, also in
 * Activity. A fix the repository moved away from is looked for again, without an error.
 */
export async function applyConflictFix(component: FixComponent): Promise<void> {
  const { fix } = conflictFixStore.getState()
  const repositoryPath = branchSyncStore.getState().syncedRepositoryPath
  if (fix === null || repositoryPath === null) {
    return
  }
  const subjectOf = subjectOfCommit()
  const description = component.steps.map(step => describeFixStep(step, subjectOf)).join("; ")
  const done = await runRewriteIntent(
    {
      label: "Applying the fix",
      surface: APPLY_FIX_SURFACE,
      repositoryPath,
      onWritten: outcome => {
        if (outcome.result.kind === "rewrite" && outcome.result.rewrite === "conflictFixApplied" && outcome.result.answer.kind === "applied") {
          revealFixed(component, outcome.result.answer.result)
          conflictFixStore.setState({ key: null, status: "idle", fix: null })
        }
      },
    },
    witnessed(repositoryPath, { rewrite: "applyConflictFix", fingerprint: fix.fingerprint, plan: component.plan }),
    "Cannot apply the fix",
  )
  if (done === null || done.result.rewrite !== "conflictFixApplied") {
    return
  }
  const token = done.undo?.token
  if (done.result.answer.kind === "stale" || token === undefined) {
    void requestConflictFix(true)
    return
  }
  showUndoToast(repositoryPath, fixTitle(component), () => undoRegroup(repositoryPath, token, "Cannot undo the fix", "Undoing the fix"), description, done.outcome)
}

/** Reveals the first branch `component` fills with the commits it moved there, as the regroup `moved` reports them */
function revealFixed(component: FixComponent, moved: MoveCommitsResult): void {
  const first = component.steps[0]
  const commits = new Set(component.steps.flatMap(step => step.commits))
  requestReveal(
    first === undefined
      ? null
      : {
          branchName: first.kind === "restack" ? first.fixes : first.to,
          commitIds: moved.commits.filter(commit => commits.has(commit.oldId)).map(commit => commit.newId),
        },
  )
}
