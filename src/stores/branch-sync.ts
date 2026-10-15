import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { Channel } from "@tauri-apps/api/core"
import type { AutosquashPlan, BranchError, BranchOrigin, BranchStack, BranchSyncStatus, Commit, CommitSyncStatus, DroppedCommit, GroupedBranchInfo, RemoteStatusUpdate, SyncEvent, TransferProgress, UpdateCheck } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { getSubjectLine, parseSubject } from "@/lib/commit"
import { UserError } from "@/lib/git/vcs-request"
import { updateFromArchivedNames, updateFromIntegrationInfo } from "@/stores/archived-branches"
import { hideAfterSync, setLandedStatus } from "@/stores/landed"
import { getLoadingPromise, getSelectedProject, useSelectedProject, vcsRequestFactory } from "@/stores/repository"
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
  /** The remote branch is an ancestor of the local one; a rebased branch is not, its push is a force push */
  fastForward: boolean
  /** Unix timestamp, 0 = never pushed */
  lastPushTime: number
  /** The remote commit the status was computed against, null without a remote branch; a push replaces only it */
  remoteTip: string | null
  /** The newest of the remote commits a push drops, newest first; `commitsBehind` counts them all */
  droppedCommits: readonly DroppedCommit[]
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
  readonly allCommitsHaveIssueReferences: boolean
  /** Most frequent author email in this branch's commits */
  readonly myEmail: string | null
  /** Grouped by an explicit `(name)` prefix, or by the issue key of the subjects */
  readonly origin: BranchOrigin
  /** The issue the branch belongs to: its own name for an issue branch, the key all commits carry for a prefix branch */
  readonly issueKey: string | null
  /** Where the branch sits in its stack; null without stacked branches */
  readonly stack: BranchStack | null
}

type MutableBranch = { -readonly [K in keyof Branch]: Branch[K] }

/** Whether the checked out branch can be updated onto the last fetched baseline, as the last sync checked it. */
export interface BaselineStatus {
  /** The repository the sync checked */
  readonly repositoryPath: string
  readonly baselineBranch: string
  /** null for a local baseline (nothing to update from) or a failed check */
  readonly update: UpdateCheck | null
  /** Why the check failed, e.g. a rebase in progress */
  readonly checkError: string | null
}

export interface BranchSyncState {
  isSyncing: boolean
  /** Finished syncs, failed ones included: the sync button exposes it so a test can wait for the next sync */
  syncCount: number
  syncError: string | null
  hasCompletedSync: boolean
  /** Repository's baseline branch (e.g., "origin/master") */
  baselineBranch: string | null
  /** Branch names in backend order */
  branchOrder: string[]
  branches: Record<string, Branch>
  /**
   * Branches with a push in flight, with how far the push is (null before git reports a stage).
   * Owned by push, never touched by sync: a sync rebuilds `branches` and must not reset a running push.
   */
  pushingBranches: Readonly<Record<string, TransferProgress | null>>
  unassignedCommits: Commit[]
  /** The fixup commits and their targets, loaded after a sync that found fixup commits; null without any */
  autosquashPlan: AutosquashPlan | null
  /** null until a sync reported it */
  baselineStatus: BaselineStatus | null
  /** Stacked branches are on for the repository (`branchdeck.stacks`), as the last grouping read it */
  stacksEnabled: boolean
  /** Issue key -> branch name: the issue groups the last grouping read (`refs/branch-deck/issue-groups`) */
  issueGroups: Readonly<Record<string, string>>
}

export const branchSyncStore = createStore<BranchSyncState>(() => ({
  isSyncing: false,
  syncCount: 0,
  syncError: null,
  hasCompletedSync: false,
  baselineBranch: null,
  branchOrder: [],
  branches: {},
  pushingBranches: {},
  unassignedCommits: [],
  autosquashPlan: null,
  baselineStatus: null,
  stacksEnabled: false,
  issueGroups: {},
}))

// ---- hooks ----

export function useBranchSync<T>(selector: (state: BranchSyncState) => T): T {
  return useStore(branchSyncStore, selector)
}

/** Branch names in display order; re-renders only when the set or order of branches changes. */
export function useBranchOrder(): string[] {
  return useStore(branchSyncStore, useShallow(state => state.branchOrder))
}

/**
 * A row of the branch table:
 * - a branch, `depth` levels in: under its issue (the issue branch, or an issue row) and under the branches it stacks on;
 *   `showIssueKey` shows its issue as a chip, for a branch with no issue row above it or a stacked branch of another issue
 * - an issue row heading `branchCount` nested branches, for an issue without a branch of its own
 *
 * Rows are interned by `key`, so an unchanged row list compares shallowly.
 */
export type BranchTableRow
  = | { readonly kind: "branch", readonly key: string, readonly name: string, readonly depth: number, readonly showIssueKey: boolean }
    | { readonly kind: "issue", readonly key: string, readonly issueKey: string, readonly branchCount: number }

const internedRows = new Map<string, BranchTableRow>()

function internRow(row: BranchTableRow): BranchTableRow {
  const existing = internedRows.get(row.key)
  if (existing !== undefined) {
    return existing
  }
  // names come and go; the cache needn't keep every row ever seen
  if (internedRows.size > 10_000) {
    internedRows.clear()
  }
  internedRows.set(row.key, row)
  return row
}

function branchRow(name: string, depth: number, showIssueKey: boolean): BranchTableRow {
  return internRow({ kind: "branch", key: `b:${depth}:${showIssueKey ? 1 : 0}:${name}`, name, depth, showIssueKey })
}

/** The branch of the table that `name` stacks on; null for a root or a parent without a branch here */
function stackParentIn(branches: Record<string, Branch>, name: string): string | null {
  const parent = branches[name]?.stack?.declaredParent ?? null
  return parent !== null && parent !== name && branches[parent] !== undefined ? parent : null
}

function branchTableRowsIn(state: BranchSyncState): BranchTableRow[] {
  const { branchOrder, branches } = state
  // the branches stacked on each branch, newest first; a branch in a cycle of parents is a root
  const children = new Map<string, string[]>()
  const isRoot = (name: string) => {
    const seen = new Set<string>([name])
    let parent = stackParentIn(branches, name)
    while (parent !== null) {
      if (seen.has(parent)) {
        return true
      }
      seen.add(parent)
      parent = stackParentIn(branches, parent)
    }
    return false
  }
  for (const name of branchOrder) {
    const parent = stackParentIn(branches, name)
    if (parent !== null && !isRoot(name)) {
      const siblings = children.get(parent)
      if (siblings === undefined) {
        children.set(parent, [name])
      }
      else {
        siblings.push(name)
      }
    }
  }
  const roots = branchOrder.filter(name => stackParentIn(branches, name) === null || isRoot(name))

  // the prefix branches of each issue among the stack roots (an issue branch is named after its key)
  const families = new Map<string, string[]>()
  for (const name of roots) {
    const issueKey = branches[name]?.issueKey
    if (issueKey != null && issueKey !== name) {
      const family = families.get(issueKey)
      if (family === undefined) {
        families.set(issueKey, [name])
      }
      else {
        family.push(name)
      }
    }
  }

  const rows: BranchTableRow[] = []
  const placed = new Set<string>()
  // a branch, then the branches stacked on it, depth-first
  const placeBranch = (name: string, depth: number, showIssueKey: boolean) => {
    if (placed.has(name)) {
      return
    }
    rows.push(branchRow(name, depth, showIssueKey))
    placed.add(name)
    const issueKey = branches[name]?.issueKey ?? null
    for (const child of children.get(name) ?? []) {
      const childIssueKey = branches[child]?.issueKey ?? null
      placeBranch(child, depth + 1, childIssueKey !== null && childIssueKey !== issueKey)
    }
  }
  const placeFamily = (issueKey: string) => {
    for (const member of families.get(issueKey) ?? []) {
      placeBranch(member, 1, false)
    }
  }
  for (const name of roots) {
    if (placed.has(name)) {
      continue
    }
    const issueKey = branches[name]?.issueKey ?? null
    if (issueKey === null || issueKey === name) {
      placeBranch(name, 0, false)
      if (issueKey === name) {
        placeFamily(issueKey)
      }
      continue
    }
    // placed under the issue branch, wherever that is in the order
    if (branches[issueKey] !== undefined && roots.includes(issueKey)) {
      continue
    }
    const family = families.get(issueKey)!
    if (family.length === 1) {
      placeBranch(name, 0, true)
    }
    else {
      rows.push(internRow({ kind: "issue", key: `i:${family.length}:${issueKey}`, issueKey, branchCount: family.length }))
      placeFamily(issueKey)
    }
  }
  return rows
}

/** The branches stacked on `name`, directly or further down, each after its parent */
export function stackDescendantsIn(state: BranchSyncState, name: string): string[] {
  const descendants: string[] = []
  const pending = [name]
  while (pending.length > 0) {
    const parent = pending.pop()!
    for (const candidate of state.branchOrder) {
      if (candidate !== name && !descendants.includes(candidate) && stackParentIn(state.branches, candidate) === parent) {
        descendants.push(candidate)
        pending.push(candidate)
      }
    }
  }
  return descendants
}

/** The branches stacked directly on `name`, newest first */
export function getStackChildren(name: string): string[] {
  const { branchOrder, branches } = branchSyncStore.getState()
  return branchOrder.filter(candidate => candidate !== name && stackParentIn(branches, candidate) === name)
}

/** The branches stacked on `name`, directly or further down */
export function getStackDescendants(name: string): string[] {
  return stackDescendantsIn(branchSyncStore.getState(), name)
}

/** Whether the last grouping read prefixes as stacked branches */
export function useStacksEnabled(): boolean {
  return useStore(branchSyncStore, state => state.stacksEnabled)
}

export function isStacksEnabled(): boolean {
  return branchSyncStore.getState().stacksEnabled
}

/** The issue groups of the last grouping: issue key -> branch name */
export function useIssueGroups(): Readonly<Record<string, string>> {
  return useStore(branchSyncStore, state => state.issueGroups)
}

export function getIssueGroups(): Readonly<Record<string, string>> {
  return branchSyncStore.getState().issueGroups
}

/** Whether issue groups put commits into the branch `name` */
export function hasIssueGroup(name: string): boolean {
  return Object.values(getIssueGroups()).includes(name)
}

/** The branch table rows: newest first, the branches of an issue nested under it. */
export function useBranchTableRows(): BranchTableRow[] {
  return useStore(branchSyncStore, useShallow(branchTableRowsIn))
}

export function getBranchTableRows(): BranchTableRow[] {
  return branchTableRowsIn(branchSyncStore.getState())
}

/** A single branch; the component re-renders only when this branch changes. */
export function useBranch(name: string): Branch | undefined {
  return useStore(branchSyncStore, state => state.branches[name])
}

/** The commit the local virtual branch points to; undefined before the sync created it. Not re-rendered by other changes of the branch. */
export function useBranchTip(name: string): string | undefined {
  return useStore(branchSyncStore, state => state.branches[name]?.commits[0]?.hash)
}

/** The commit the remote branch points to; null when it is not on the remote or its status is not loaded yet */
export function useBranchRemoteTip(name: string): string | null {
  return useStore(branchSyncStore, state => state.branches[name]?.remoteStatus?.remoteTip ?? null)
}

/** The remote status of the branch, null until loaded; re-rendered only when a new status replaces it */
export function useBranchRemoteStatus(name: string): RemoteStatus | null {
  return useStore(branchSyncStore, state => state.branches[name]?.remoteStatus ?? null)
}

export function useIsBranchPushing(name: string): boolean {
  return useStore(branchSyncStore, state => Object.hasOwn(state.pushingBranches, name))
}

/** How far the running push of the branch is; null before git reports a stage and when no push runs */
export function useBranchPushProgress(name: string): TransferProgress | null {
  return useStore(branchSyncStore, state => state.pushingBranches[name] ?? null)
}

export function useUnassignedCommits(): Commit[] {
  return useStore(branchSyncStore, state => state.unassignedCommits)
}

export function useAutosquashPlan(): AutosquashPlan | null {
  return useStore(branchSyncStore, state => state.autosquashPlan)
}

export function getAutosquashPlan(): AutosquashPlan | null {
  return branchSyncStore.getState().autosquashPlan
}

/** The update state of the selected repository; null before its first sync */
export function useBaselineStatus(): BaselineStatus | null {
  const repositoryPath = useSelectedProject()?.path
  return useStore(branchSyncStore, state => state.baselineStatus?.repositoryPath === repositoryPath ? state.baselineStatus : null)
}

/** Replaces the update check of the selected repository, e.g. with the result of an update */
export function setBaselineUpdate(repositoryPath: string, update: UpdateCheck): void {
  branchSyncStore.setState(state => ({
    baselineStatus: {
      repositoryPath,
      baselineBranch: state.baselineStatus?.baselineBranch ?? update.upstream,
      update,
      checkError: null,
    },
  }))
}

export function getBranch(name: string): Branch | undefined {
  return branchSyncStore.getState().branches[name]
}

/** The root of the stack of `name`: itself without a parent branch; cycle-safe */
function stackRootIn(branches: Record<string, Branch>, name: string): string {
  const seen = new Set<string>([name])
  let root = name
  let parent = stackParentIn(branches, root)
  while (parent !== null && !seen.has(parent)) {
    seen.add(parent)
    root = parent
    parent = stackParentIn(branches, root)
  }
  return root
}

function moveTargetsIn(state: BranchSyncState, sourceBranch: string | null, limit: number): string[] {
  const source = sourceBranch === null ? undefined : state.branches[sourceBranch]
  const issueKey = source?.issueKey ?? null
  const candidates = state.branchOrder.filter(name => name !== sourceBranch)
  // `branchOrder` is newest first; the branches of the same stack go first, then those of the same issue
  const stackRoot = source?.stack == null ? null : stackRootIn(state.branches, source.name)
  const sameStack = stackRoot === null ? [] : candidates.filter(name => stackRootIn(state.branches, name) === stackRoot)
  const sameIssue = issueKey === null ? [] : candidates.filter(name => !sameStack.includes(name) && state.branches[name]?.issueKey === issueKey)
  const others = candidates.filter(name => !sameStack.includes(name) && !sameIssue.includes(name))
  return [...sameStack, ...sameIssue, ...others].slice(0, limit)
}

/**
 * Branches to offer as a move target for commits of `sourceBranch` (null: unassigned): the same stack first, then the
 * same issue, then the newest.
 */
export function useMoveTargets(sourceBranch: string | null, limit: number): string[] {
  return useStore(branchSyncStore, useShallow(state => moveTargetsIn(state, sourceBranch, limit)))
}

export function getMoveTargets(sourceBranch: string | null, limit: number): string[] {
  return moveTargetsIn(branchSyncStore.getState(), sourceBranch, limit)
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

export function isBranchPushing(name: string): boolean {
  return Object.hasOwn(branchSyncStore.getState().pushingBranches, name)
}

export function setBranchPushing(name: string, isPushing: boolean): void {
  branchSyncStore.setState((state) => {
    if (Object.hasOwn(state.pushingBranches, name) === isPushing) {
      return state
    }
    if (isPushing) {
      return { pushingBranches: { ...state.pushingBranches, [name]: null } }
    }
    const { [name]: _, ...pushingBranches } = state.pushingBranches
    return { pushingBranches }
  })
}

/** Records how far the push of the branch is; a message that arrives after the push ended is dropped */
export function setBranchPushProgress(name: string, progress: TransferProgress): void {
  branchSyncStore.setState(state => Object.hasOwn(state.pushingBranches, name)
    ? { pushingBranches: { ...state.pushingBranches, [name]: progress } }
    : state)
}

export function setBranchRemoteStatus(name: string, remoteStatus: RemoteStatus): void {
  updateBranch(name, branch => ({ ...branch, remoteStatus }))
}

type RemoteStatusFields = Pick<RemoteStatusUpdate, "remoteExists" | "unpushedCommits" | "commitsBehind" | "myUnpushedCount" | "fastForward" | "lastPushTime" | "remoteTip" | "droppedCommits">

export function toRemoteStatus(update: RemoteStatusFields): RemoteStatus {
  return {
    exists: update.remoteExists,
    unpushedCommits: update.unpushedCommits,
    commitsAhead: update.unpushedCommits.length,
    commitsBehind: update.commitsBehind,
    myCommitsAhead: update.myUnpushedCount ?? 0,
    fastForward: update.fastForward,
    lastPushTime: update.lastPushTime ?? 0,
    remoteTip: update.remoteTip,
    droppedCommits: update.droppedCommits,
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

    const vcsRequest = vcsRequestFactory.createRequest()
    const channel = new Channel<SyncEvent>()
    channel.onmessage = (event) => {
      handleSyncEvent(event, vcsRequest.repositoryPath, options)
    }

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
    hideAfterSync(vcsRequest.repositoryPath)
    void loadAutosquashPlan(vcsRequest.repositoryPath)
  }
  catch (error) {
    branchSyncStore.setState({ syncError: (error instanceof Error ? error.message : String(error)) || "Failed to sync branches" })
    console.error("Sync error:", error)
  }
  finally {
    branchSyncStore.setState(state => ({ isSyncing: false, syncCount: state.syncCount + 1 }))
  }
}

function isFixupCommit(commit: Commit): boolean {
  return parseSubject(getSubjectLine(commit.message)).autosquash !== ""
}

let autosquashPlanRequest = 0

/** Loads the plan only if a commit is a fixup: most syncs find none and need no extra git process. */
async function loadAutosquashPlan(repositoryPath: string): Promise<void> {
  const request = ++autosquashPlanRequest
  const { branches, unassignedCommits } = branchSyncStore.getState()
  const hasFixups = unassignedCommits.some(isFixupCommit) || Object.values(branches).some(branch => branch.commits.some(isFixupCommit))
  if (!hasFixups) {
    branchSyncStore.setState({ autosquashPlan: null })
    return
  }
  const result = await invokeCommand(() => commands.getAutosquashPlan({ repositoryPath }))
  // a later sync started its own request
  if (request !== autosquashPlanRequest) {
    return
  }
  if (result.status === "error") {
    console.error("Failed to load the autosquash plan:", result.error)
    branchSyncStore.setState({ autosquashPlan: null })
    return
  }
  branchSyncStore.setState({ autosquashPlan: result.data })
}

function handleSyncEvent(event: SyncEvent, repositoryPath: string, options?: SyncOptions): void {
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
      updateFromArchivedNames(event.data.branchNames, event.data.newlyArchived)
      break
    case "branchIntegrationDetected":
      updateFromIntegrationInfo(event.data.info)
      break
    case "remoteStatusUpdate":
      // the push result is authoritative: a status computed by the sync mid-push is stale by the time the push ends
      if (!isBranchPushing(event.data.branchName)) {
        setBranchRemoteStatus(event.data.branchName, toRemoteStatus(event.data))
      }
      break
    case "baselineStatus": {
      const { landing, landed, ...status } = event.data
      branchSyncStore.setState({ baselineStatus: { repositoryPath, ...status } })
      setLandedStatus(repositoryPath, landing, landed, status.update?.upstream ?? status.baselineBranch)
      break
    }
    case "commitsBlocked":
      // not displayed
      break
  }
}

function createBranchFromGroup(info: GroupedBranchInfo, remoteStatus: RemoteStatus | null): Branch {
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
    remoteStatus,
    allCommitsHaveIssueReferences: info.allCommitsHaveIssueReferences,
    myEmail: info.myEmail ?? null,
    origin: info.origin,
    issueKey: info.issueKey ?? null,
    stack: info.stack ?? null,
  }
}

/**
 * Reconciles the branch list with the grouping result: backend order, every branch (re)starts in the syncing state
 * (existing ones are reset, which a fresh record keyed by the same name does), branches no longer present are dropped.
 * The last known remote status is kept until the sync reports a fresh one, so the push button label does not flicker.
 */
function handleBranchesGrouped(data: Extract<SyncEvent, { type: "branchesGrouped" }>["data"]): void {
  const previous = branchSyncStore.getState().branches
  const branches: Record<string, Branch> = {}
  const branchOrder: string[] = []
  for (const info of data.branches) {
    if (info.name in branches) {
      continue
    }
    branches[info.name] = createBranchFromGroup(info, previous[info.name]?.remoteStatus ?? null)
    branchOrder.push(info.name)
  }
  branchSyncStore.setState({ baselineBranch: data.baselineBranch, branches, branchOrder, stacksEnabled: data.stacks ?? false, issueGroups: data.issueGroups ?? {} })
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
    if (status === "Blocked") {
      // not an error of this branch: its parent can't be built on; the reason is the tooltip
      return { ...branch, status, statusText: error?.Generic ?? "blocked" }
    }
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
