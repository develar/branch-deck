import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { Channel } from "@tauri-apps/api/core"
import type { BranchError, BranchSyncStatus, Commit, CommitSyncStatus, GroupedBranchInfo, RemoteStatusUpdate, SyncEvent } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { UserError } from "@/lib/git/vcs-request"
import { updateFromArchivedNames, updateFromIntegrationInfo } from "@/stores/archived-branches"
import { getLoadingPromise, getSelectedProject, vcsRequestFactory } from "@/stores/repository"
import { updateProject } from "@/stores/settings/repositorySettings"

export interface SyncOptions {
  autoScroll?: boolean
  autoExpand?: boolean
  targetBranchName?: string
}

/** Commit augmented with its sync outcome. */
export interface SyncedCommit extends Commit {
  /** New hash after sync */
  hash?: string
  status?: CommitSyncStatus
  error?: BranchError | null
}

/** Remote tracking information for a branch. */
export interface RemoteStatus {
  exists: boolean
  unpushedCommits: string[]
  commitsAhead: number
  commitsBehind: number
  myCommitsAhead: number
  /** Unix timestamp, 0 = never pushed */
  lastPushTime: number
}

export type BranchStatus = "Syncing" | BranchSyncStatus

/**
 * Virtual branch, updated incrementally by sync events.
 * Immutable: every update creates a new record, so a component selecting one branch re-renders only when that branch changes.
 */
export interface Branch {
  readonly name: string
  readonly commits: readonly SyncedCommit[]
  /** Original commit hash → index in `commits` */
  readonly commitIndex: Readonly<Record<string, number>>
  readonly commitCount: number
  readonly status: BranchStatus
  readonly statusText: string
  readonly processedCount: number
  readonly hasError: boolean
  readonly errorDetails?: BranchError
  readonly autoExpandRequested: boolean
  readonly autoScrollRequested: boolean
  readonly latestCommitTime: number
  readonly summary: string
  /** null = not yet loaded */
  readonly remoteStatus: RemoteStatus | null
  readonly isPushing: boolean
  readonly allCommitsHaveIssueReferences: boolean
  /** Most frequent author email in this branch's commits */
  readonly myEmail: string | null
}

type MutableBranch = { -readonly [K in keyof Branch]: Branch[K] }

export interface BranchSyncState {
  isSyncing: boolean
  syncError: string | null
  hasCompletedSync: boolean
  /** Repository's baseline branch (e.g., "origin/master") */
  baselineBranch: string | null
  /** Branch names in backend order */
  branchOrder: string[]
  branches: Record<string, Branch>
  unassignedCommits: Commit[]
}

export const branchSyncStore = createStore<BranchSyncState>(() => ({
  isSyncing: false,
  syncError: null,
  hasCompletedSync: false,
  baselineBranch: null,
  branchOrder: [],
  branches: {},
  unassignedCommits: [],
}))

// ---- hooks ----

export function useBranchSync<T>(selector: (state: BranchSyncState) => T): T {
  return useStore(branchSyncStore, selector)
}

/** Branch names in display order; re-renders only when the set or order of branches changes. */
export function useBranchOrder(): string[] {
  return useStore(branchSyncStore, useShallow(state => state.branchOrder))
}

/** A single branch; the component re-renders only when this branch changes. */
export function useBranch(name: string): Branch | undefined {
  return useStore(branchSyncStore, state => state.branches[name])
}

export function useUnassignedCommits(): Commit[] {
  return useStore(branchSyncStore, state => state.unassignedCommits)
}

export function getBranch(name: string): Branch | undefined {
  return branchSyncStore.getState().branches[name]
}

// ---- branch updates ----

/** Immutable update of a single branch; no-op if the branch is unknown or the updater returns the same record. */
function updateBranch(name: string, updater: (branch: Branch) => Branch): void {
  branchSyncStore.setState((state) => {
    const branch = state.branches[name]
    if (branch === undefined) {
      return state
    }
    const next = updater(branch)
    return next === branch ? state : { branches: { ...state.branches, [name]: next } }
  })
}

/** Clears the auto-expand/auto-scroll requests once a component has acted on them. */
export function consumeAutoRequests(name: string): void {
  updateBranch(name, branch => (branch.autoExpandRequested || branch.autoScrollRequested)
    ? { ...branch, autoExpandRequested: false, autoScrollRequested: false }
    : branch)
}

export function setBranchPushing(name: string, isPushing: boolean): void {
  updateBranch(name, branch => branch.isPushing === isPushing ? branch : { ...branch, isPushing })
}

export function setBranchRemoteStatus(name: string, remoteStatus: RemoteStatus): void {
  updateBranch(name, branch => ({ ...branch, remoteStatus }))
}

type RemoteStatusFields = Pick<RemoteStatusUpdate, "remoteExists" | "unpushedCommits" | "commitsBehind" | "myUnpushedCount" | "lastPushTime">

export function toRemoteStatus(update: RemoteStatusFields): RemoteStatus {
  return {
    exists: update.remoteExists,
    unpushedCommits: update.unpushedCommits,
    commitsAhead: update.unpushedCommits.length,
    commitsBehind: update.commitsBehind,
    myCommitsAhead: update.myUnpushedCount ?? 0,
    lastPushTime: update.lastPushTime ?? 0,
  }
}

function updateCommit(branch: Branch, originalHash: string, patch: Partial<SyncedCommit>): readonly SyncedCommit[] {
  const index = branch.commitIndex[originalHash]
  if (index === undefined) {
    return branch.commits
  }
  return branch.commits.with(index, { ...branch.commits[index]!, ...patch })
}

// ---- sync ----

/** Runs a full sync of the selected repository, streaming progress events into the store. */
export async function syncBranches(options?: SyncOptions): Promise<void> {
  // archived branches are not cleared: they are reconciled by name
  branchSyncStore.setState({ isSyncing: true, syncError: null })

  try {
    // wait for a pending branch prefix load, so the sync does not run with a stale prefix
    const loadingPromise = getLoadingPromise()
    if (loadingPromise !== null) {
      let timer: ReturnType<typeof setTimeout> | undefined
      try {
        await Promise.race([
          loadingPromise,
          new Promise((_, reject) => {
            timer = setTimeout(() => reject(new UserError("Timeout waiting for branch prefix configuration")), 5000)
          }),
        ])
      }
      finally {
        clearTimeout(timer)
      }
    }

    const channel = new Channel<SyncEvent>()
    channel.onmessage = (event) => {
      handleSyncEvent(event, options)
    }

    const vcsRequest = vcsRequestFactory.createRequest()
    const result = await commands.syncBranches(vcsRequest, channel)
    if (result.status === "error") {
      branchSyncStore.setState({ syncError: result.error.message || "Failed to sync branches" })
      console.error("Sync error:", result.error)
      return
    }

    const project = getSelectedProject()
    if (project !== null) {
      updateProject(project.path, {
        lastSyncTime: Date.now(),
        lastBranchCount: branchSyncStore.getState().branchOrder.length,
      })
    }
    branchSyncStore.setState({ hasCompletedSync: true })
  }
  catch (error) {
    branchSyncStore.setState({ syncError: (error instanceof Error ? error.message : String(error)) || "Failed to sync branches" })
    console.error("Sync error:", error)
  }
  finally {
    branchSyncStore.setState({ isSyncing: false })
  }
}

function handleSyncEvent(event: SyncEvent, options?: SyncOptions): void {
  switch (event.type) {
    case "issueNavigationConfig": {
      const project = getSelectedProject()
      if (project !== null && event.data.config) {
        updateProject(project.path, { issueNavigationConfig: event.data.config })
      }
      break
    }
    case "branchesGrouped":
      handleBranchesGrouped(event.data)
      break
    case "commitSynced":
      handleCommitSynced(event.data)
      break
    case "commitError":
      handleCommitError(event.data, options)
      break
    case "branchStatusUpdate":
      handleBranchStatusUpdate(event.data, options)
      break
    case "unassignedCommits":
      branchSyncStore.setState({ unassignedCommits: event.data.commits })
      break
    case "archivedBranchesFound":
      updateFromArchivedNames(event.data.branchNames)
      break
    case "branchIntegrationDetected":
      updateFromIntegrationInfo(event.data.info)
      break
    case "remoteStatusUpdate":
      setBranchRemoteStatus(event.data.branchName, toRemoteStatus(event.data))
      break
    case "commitsBlocked":
      // not displayed
      break
  }
}

function createBranchFromGroup(info: GroupedBranchInfo): Branch {
  const commitIndex: Record<string, number> = {}
  info.commits.forEach((commit, index) => {
    commitIndex[commit.originalHash] = index
  })
  return {
    name: info.name,
    commits: info.commits,
    commitIndex,
    commitCount: info.commits.length,
    status: "Syncing",
    statusText: "syncing…",
    processedCount: 0,
    hasError: false,
    errorDetails: undefined,
    autoExpandRequested: false,
    autoScrollRequested: false,
    latestCommitTime: info.latestCommitTime,
    summary: info.summary,
    remoteStatus: null,
    isPushing: false,
    allCommitsHaveIssueReferences: info.allCommitsHaveIssueReferences,
    myEmail: info.myEmail ?? null,
  }
}

/**
 * Reconciles the branch list with the grouping result: backend order, every branch (re)starts in the syncing state
 * (existing ones are reset, which a fresh record keyed by the same name does), branches no longer present are dropped.
 */
function handleBranchesGrouped(data: Extract<SyncEvent, { type: "branchesGrouped" }>["data"]): void {
  const branches: Record<string, Branch> = {}
  const branchOrder: string[] = []
  for (const info of data.branches) {
    if (info.name in branches) {
      continue
    }
    branches[info.name] = createBranchFromGroup(info)
    branchOrder.push(info.name)
  }
  branchSyncStore.setState({ baselineBranch: data.baselineBranch, branches, branchOrder })
}

function handleCommitSynced(data: Extract<SyncEvent, { type: "commitSynced" }>["data"]): void {
  const { branchName, commitHash, newHash, status } = data
  updateBranch(branchName, (branch) => {
    if (branch.commitIndex[commitHash] === undefined) {
      return branch
    }
    return {
      ...branch,
      commits: updateCommit(branch, commitHash, { hash: newHash, status }),
      processedCount: branch.processedCount + 1,
    }
  })
}

function handleCommitError(data: Extract<SyncEvent, { type: "commitError" }>["data"], options?: SyncOptions): void {
  const { branchName, commitHash, error } = data
  const autoExpand = options?.autoExpand ?? true
  updateBranch(branchName, (branch) => {
    const next: MutableBranch = {
      ...branch,
      // errors count as processed too
      processedCount: branch.processedCount + 1,
      commits: updateCommit(branch, commitHash, { status: "Error", error }),
      statusText: error.MergeConflict ? "merge conflict" : (error.Generic ?? ""),
      status: "Error",
      hasError: true,
      errorDetails: error,
    }
    if (autoExpand) {
      next.autoExpandRequested = true
      next.autoScrollRequested = options?.autoScroll ?? true
    }
    return next
  })
}

function getStatusText(status: BranchSyncStatus): string {
  switch (status) {
    case "MergeConflict":
      return "merge conflict"
    case "AnalyzingConflict":
      return "analyzing conflict…"
    case "Error":
      return "internal error"
    default:
      return status.toLowerCase()
  }
}

function shouldRequestAutoExpand(branchName: string, status: BranchSyncStatus, options?: SyncOptions): boolean {
  if (!(options?.autoExpand ?? true)) {
    return false
  }
  // with a target branch, only that branch is expanded
  if (options?.targetBranchName) {
    return branchName === options.targetBranchName && (status === "Created" || status === "Updated")
  }
  // errors, conflicts and meaningful changes
  return status === "Error" || status === "MergeConflict" || status === "Created" || status === "Updated"
}

function handleBranchStatusUpdate(data: Extract<SyncEvent, { type: "branchStatusUpdate" }>["data"], options?: SyncOptions): void {
  const { branchName, status, error } = data
  updateBranch(branchName, (branch) => {
    const next: MutableBranch = { ...branch, status, statusText: getStatusText(status) }
    if (error) {
      next.errorDetails = error
      next.hasError = true
    }
    if (status === "MergeConflict" || status === "Error") {
      next.hasError = true
    }
    if (shouldRequestAutoExpand(branchName, status, options)) {
      next.autoExpandRequested = true
      next.autoScrollRequested = options?.autoScroll ?? true
    }
    return next
  })
}
