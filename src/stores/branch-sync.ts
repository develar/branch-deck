import { useMemo } from "react"
import { Channel } from "@tauri-apps/api/core"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { createStore } from "zustand/vanilla"
import type {
  AutosquashPlan,
  BranchError,
  BranchIntegrationInfo,
  BranchOrigin,
  BranchStack,
  BranchSyncStatus,
  Commit,
  CommitSyncStatus,
  DroppedCommit,
  GroupedBranchInfo,
  RemoteStatusUpdate,
  StackBranchConflict,
  StackBranchState,
  StackBranchUpdate,
  StackMemberBlock,
  SyncEvent,
  UpdateCheck,
  WordResolutionSummary,
} from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { getSubjectLine, parseSubject } from "@/lib/commit"
import { UserError } from "@/lib/git/vcs-request"
import type { ParkedState } from "@/lib/parked"
import { collectParkedStates } from "@/lib/parked"
import { sameStringRecord } from "@/lib/record"
import { stackIslandRoots } from "@/lib/stack-collapse"
import type { StackLanes } from "@/lib/stack-lanes"
import { layoutStack } from "@/lib/stack-lanes"
import { showToast } from "@/lib/toast-message"
import type { PushProgress } from "@/lib/transfer-progress"
import { addTiming } from "@/lib/update-progress"
import { stackReadinessKey } from "@/lib/whole-stack"
import { resetArchivedBranches, settleArchivedPlaceholders, updateFromArchivedNames, updateFromIntegrationInfos } from "@/stores/archived-branches"
import { hideAfterSync, setLandedStatus } from "@/stores/landed"
import { getSelectedProject, getSelectedRepositoryPath, isSelectedRepository, useSelectedProjectPath } from "@/stores/repository"
import { updateProject } from "@/stores/settings/repositorySettings"
import { resetUnappliedBranches, setUnappliedBranches } from "@/stores/unapplied-branches"

export interface SyncOptions {
  autoScroll?: boolean
  autoExpand?: boolean
  targetBranchName?: string
  /**
   * The user asked for it (Sync, its menu item and key, the Ready to Sync card): a sync git defers (a rebase in
   * progress, a detached HEAD) says so in an `info` toast; any other sync waits silently
   */
  manual?: boolean
}

/** Commit augmented with its sync outcome. */
export interface SyncedCommit extends Commit {
  /** New hash after sync */
  hash?: string
  status?: CommitSyncStatus
  error?: BranchError | null
  /** The conflict of git that the copy resolved by words (`branchdeck.resolveByWords`) */
  resolved?: WordResolutionSummary | null
}

/** Remote tracking information for a branch. */
export interface RemoteStatus {
  exists: boolean
  unpushedCommits: string[]
  commitsAhead: number
  /** Remote commits whose content the branch lacks that are no older version of a commit it has: a push drops them */
  commitsBehind: number
  /** Remote commits that are older versions of commits amended here: a push replaces them */
  commitsAmended: number
  /** Remote commits that are older versions of the commits of the branches it is built on: a push replaces them */
  commitsRestacked: number
  /** Remote commits that are older versions of commits the baseline has (they landed, maybe with other changes): a push replaces them */
  commitsLanded: number
  /** Commits the remote has by content but with another message or author (reworded here): a push replaces their remote copies */
  commitsReworded: number
  /**
   * Commits of the branches a stacked branch is built on that its remote branch lacks (a new commit of the parent since
   * its push): a Safe Push of the remote branch lands an older piece, so a push replaces it
   */
  ancestorCommitsAhead: number
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
  /** The title of the branch's description (`bd describe`), shown as written instead of the summary; null without one */
  readonly descriptionTitle: string | null
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

/**
 * The stack branch of a stack that forks (`{prefix}/stack/<root>`, `getFullStackBranchName`): all its commits in one more
 * virtual branch, which a Safe Push of the whole stack lands in one robot, as the last sync reported it. Only a `Built`
 * one has the stack's commits as they are; no push may send another (spec/branch/whole-stack.spec.md).
 */
export interface StackBranch {
  readonly root: string
  /** `{prefix}/stack/<root>` */
  readonly branchName: string
  /** The branches of the stack, the root first, each after its parent; empty for one kept for its remote branch */
  readonly members: readonly string[]
  readonly commitCount: number
  readonly state: StackBranchState
  /** The tip the sync left: its build for `Built`, else the tip the ref kept; null without a ref */
  readonly tip: string | null
  /** `Blocked`: the member that failed or is blocked */
  readonly blockedBy: StackMemberBlock | null
  /** `Conflict`: the commit that conflicts and its files */
  readonly conflict: StackBranchConflict | null
  /** `Stale`: why the ref is not the build of the stack's commits */
  readonly error: string | null
  /** null while the sync didn't compute it */
  readonly remoteStatus: RemoteStatus | null
}

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
  /** A chain of syncs runs (`syncBranches`): true from its first run to the end of its follow-up */
  isSyncing: boolean
  /**
   * Finished sync runs, failed ones included: the sync button exposes it so a test can wait for the next sync. A sync
   * and its follow-up count two, `isSyncing` stays true between them.
   */
  syncCount: number
  syncError: string | null
  /** The repository whose branches the store holds, set by its first successful sync; null before it */
  syncedRepositoryPath: string | null
  /**
   * The fingerprint of what the last successful sync read (`sync_inputs_fingerprint`), taken when it started, so a
   * commit made while it ran counts as a change; a focus syncs only when it differs (`syncIfChanged`)
   */
  syncedInputs: { readonly repositoryPath: string; readonly fingerprint: string } | null
  /** When a focus last found the inputs as the last sync read them (ms): the header says "Checked just now" then */
  inputsCheckedAt: number | null
  /** Repository's baseline branch (e.g., "origin/master") */
  baselineBranch: string | null
  /** Branch names in backend order */
  branchOrder: string[]
  branches: Record<string, Branch>
  /**
   * Branches with a push in flight, and the stack branch of the whole stack of a root under its key `stack:<root>`
   * (`stackReadinessKey`, a name no branch has), by `pushKey` of the repository the push started in: a push that ends
   * after a switch of the repository marks nothing of the branch of the same name there; how far each is lives in
   * `pushProgressStore`, so its ticks don't re-run the selectors of this store. Owned by push, never touched by sync: a
   * sync rebuilds `branches` and must not reset a running push.
   */
  pushingBranches: Readonly<Record<string, true>>
  /**
   * The stack pushes in flight: the branch a push of several branches started from (Push Stack, Push with Children,
   * Push Up to Here) -> the branches it sends in its one `git push`, parents first; empty while it waits for a sync.
   * By `pushKey` of the repository the push started in, and owned by push, as `pushingBranches`.
   */
  stackPushes: Readonly<Record<string, readonly string[]>>
  unassignedCommits: Commit[]
  /** The fixup commits and their targets, loaded after a sync that found fixup commits; null without any */
  autosquashPlan: AutosquashPlan | null
  /** null until a sync reported it */
  baselineStatus: BaselineStatus | null
  /** The sync run (`requestSync`) whose grouping `branches` holds: their counts tell how far that run is; 0 before one */
  groupedRun: number
  /** Stacked branches are on for the repository (`branchdeck.stacks`), as the last grouping read it */
  stacksEnabled: boolean
  /** Issue key -> branch name: the issue groups the last grouping read (`refs/branch-deck/issue-groups`) */
  issueGroups: Readonly<Record<string, string>>
  /** Child -> parent: the stack parents the last grouping read (`refs/branch-deck/stack-parents`); empty with stacks off */
  stackParents: Readonly<Record<string, string>>
  /**
   * Branch -> note: the parked branches the last grouping read (`refs/branch-deck/parked`), kept local, never pushed;
   * also the lines of branches without commits here
   */
  parked: Readonly<Record<string, string>>
  /**
   * Root -> its stack branch, as the last sync reported it. A grouping keeps the ones it announces, as they were until
   * their update comes, and drops the others.
   */
  stackBranches: Readonly<Record<string, StackBranch>>
}

export const branchSyncStore = createStore<BranchSyncState>(() => ({
  isSyncing: false,
  syncCount: 0,
  syncError: null,
  syncedRepositoryPath: null,
  syncedInputs: null,
  inputsCheckedAt: null,
  baselineBranch: null,
  branchOrder: [],
  branches: {},
  pushingBranches: {},
  stackPushes: {},
  unassignedCommits: [],
  autosquashPlan: null,
  baselineStatus: null,
  groupedRun: 0,
  stacksEnabled: false,
  issueGroups: {},
  stackParents: {},
  parked: {},
  stackBranches: {},
}))

/**
 * How far each running push is, by `pushKey` of the repository it started in, absent before git reports a stage: apart
 * from `branchSyncStore`, because a progress tick would re-run every selector of it (docs/design/rendering-performance.md)
 */
const pushProgressStore = createStore<Readonly<Record<string, PushProgress>>>(() => ({}))

// ---- hooks ----

export function useBranchSync<T>(selector: (state: BranchSyncState) => T): T {
  return useStore(branchSyncStore, selector)
}

/** Branch names in display order; re-renders only when the set or order of branches changes. */
export function useBranchOrder(): string[] {
  return useStore(
    branchSyncStore,
    useShallow(state => state.branchOrder),
  )
}

/**
 * A row of the branch table:
 * - a branch, `depth` levels in: under its issue (the issue branch, or an issue row) and under the branches it stacks on;
 *   `showIssueKey` shows its issue as a chip, for a branch with no issue row above it or a stacked branch of another issue
 *   (unless its name is that issue); `familyIndent` 1 under its issue, the one indent level of the table; `lanes` its
 *   place in the stack graph, null on every row while no branch is stacked on another; `dimmed` a branch the filter
 *   didn't match, shown because a match stacks on it
 * - an issue row heading its `members` (the family and the stacks on it), for an issue without a branch of its own
 *
 * Rows are interned by `key`, which encodes every field, so an unchanged row list compares shallowly.
 */
export type BranchTableRow =
  | {
      readonly kind: "branch"
      readonly key: string
      readonly name: string
      readonly depth: number
      readonly showIssueKey: boolean
      readonly familyIndent: 0 | 1
      readonly lanes: StackLanes
      readonly dimmed: boolean
    }
  | { readonly kind: "issue"; readonly key: string; readonly issueKey: string; readonly members: readonly string[] }

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

function lanesKey(lanes: StackLanes): string {
  return `${lanes.lane},${lanes.forkFrom ?? ""},${lanes.up ? 1 : 0}${lanes.down ? 1 : 0},${lanes.through.join(".")}${lanes.alone === true ? "a" : ""}`
}

/** The lanes of a branch in a table without stacks: the node alone */
function aloneLanes(lanes: StackLanes): StackLanes {
  return { ...lanes, alone: true }
}

function branchRow(name: string, depth: number, showIssueKey: boolean, familyIndent: 0 | 1, lanes: StackLanes, dimmed: boolean): BranchTableRow {
  const key = `b:${depth}:${showIssueKey ? 1 : 0}:${familyIndent}:${dimmed ? 1 : 0}:${lanesKey(lanes)}:${name}`
  return internRow({ kind: "branch", key, name, depth, showIssueKey, familyIndent, lanes, dimmed })
}

/** The branch of the table that `name` stacks on; null for a root or a parent without a branch here */
function stackParentIn(branches: Record<string, Branch>, name: string): string | null {
  const parent = branches[name]?.stack?.declaredParent ?? null
  return parent !== null && parent !== name && branches[parent] !== undefined ? parent : null
}

/**
 * What the table rows and the stack lookups are computed from: the order, per branch its parent and issue, and the parked
 * branches. A sync event (a commit synced, a remote status) changes the `branches` record but not this.
 */
interface BranchStructure {
  readonly branchOrder: readonly string[]
  readonly branches: Record<string, Branch>
  readonly parked: Readonly<Record<string, string>>
  readonly key: string
  /** The branches stacked directly on each branch, in branch order; cycles included */
  readonly children: ReadonlyMap<string, readonly string[]>
  /** The parked branches and those built on one, which carry its commits (`collectParkedStates`) */
  readonly parkedStates: ReadonlyMap<string, ParkedState>
  /** The branches of the main table, in branch order: the others are parked */
  readonly mainOrder: readonly string[]
}

/** The branch `name` is built on, whose commits it carries: its effective base, else the declared parent of a blocked one */
function buildParentIn(branches: Record<string, Branch>, name: string): string | null {
  const stack = branches[name]?.stack
  return stack?.effectiveBase ?? stack?.declaredParent ?? null
}

let lastStructure: BranchStructure | null = null

/** The structure of `state`, the same object while the structure stays the same */
function structureOf(state: BranchSyncState): BranchStructure {
  const { branchOrder, branches, parked } = state
  const last = lastStructure
  if (last !== null && last.branchOrder === branchOrder && last.branches === branches && last.parked === parked) {
    return last
  }
  const parkedStates = collectParkedStates(branchOrder, parked, name => buildParentIn(branches, name))
  let key = ""
  for (const name of branchOrder) {
    const branch = branches[name]
    const state = parkedStates.get(name)
    const parkedKey = state === undefined ? "" : `${state.with ?? ""}\0${state.note}`
    key += branch === undefined ? `${name}\0?\n` : `${name}\0${stackParentIn(branches, name) ?? ""}\0${branch.issueKey ?? ""}\0${parkedKey}\n`
  }
  if (last !== null && last.key === key) {
    lastStructure = { ...last, branchOrder, branches, parked }
    return lastStructure
  }
  const children = new Map<string, string[]>()
  for (const name of branchOrder) {
    const parent = stackParentIn(branches, name)
    if (parent !== null) {
      const siblings = children.get(parent)
      if (siblings === undefined) {
        children.set(parent, [name])
      } else {
        siblings.push(name)
      }
    }
  }
  const mainOrder = parkedStates.size === 0 ? branchOrder : branchOrder.filter(name => !parkedStates.has(name))
  lastStructure = { branchOrder, branches, parked, key, children, parkedStates, mainOrder }
  return lastStructure
}

/** The branches of `visible` and the branches they stack on, in branch order */
function withStackAncestors(state: BranchSyncState, visible: ReadonlySet<string>): string[] {
  const kept = new Set<string>()
  for (const name of visible) {
    let current: string | null = state.branches[name] === undefined ? null : name
    while (current !== null && !kept.has(current)) {
      kept.add(current)
      current = stackParentIn(state.branches, current)
    }
  }
  return state.branchOrder.filter(name => kept.has(name))
}

/** The table of a branch: the main one, or the Parked card of the branches kept local */
export type BranchTableSection = "main" | "parked"

/** The branches of `section` among `branches`: the same record without parked branches */
function sectionBranches(structure: BranchStructure, section: BranchTableSection): Record<string, Branch> {
  if (structure.parkedStates.size === 0) {
    return structure.branches
  }
  const parked = section === "parked"
  return Object.fromEntries(Object.entries(structure.branches).filter(([name]) => structure.parkedStates.has(name) === parked))
}

/** The rows of `branchTableRowsIn`, computed: the stack graph is laid out over the rows that remain */
function computeBranchTableRows(state: BranchSyncState, visible: ReadonlySet<string> | null, section: BranchTableSection = "main"): BranchTableRow[] {
  const structure = structureOf(state)
  // a branch parked with its parent stays under it in the Parked card; a parked child of a branch to land is a root there
  const branches = sectionBranches(structure, section)
  const branchOrder = (visible === null ? state.branchOrder : withStackAncestors(state, visible)).filter(name => branches[name] !== undefined)
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
      } else {
        siblings.push(name)
      }
    }
  }
  const roots = branchOrder.filter(name => stackParentIn(branches, name) === null || isRoot(name))
  const rootSet = new Set(roots)
  // without a branch stacked on another the table has no graph: a lane for the node of each branch
  const hasStacks = children.size > 0

  // the prefix branches of each issue among the stack roots (an issue branch is named after its key)
  const families = new Map<string, string[]>()
  for (const name of roots) {
    const issueKey = branches[name]?.issueKey
    if (issueKey != null && issueKey !== name) {
      const family = families.get(issueKey)
      if (family === undefined) {
        families.set(issueKey, [name])
      } else {
        family.push(name)
      }
    }
  }

  const rows: BranchTableRow[] = []
  const placed = new Set<string>()
  // a stack root, then the branches stacked on it, depth-first, on the lanes of the stack graph
  const placeStack = (root: string, familyIndent: 0 | 1, showRootIssueKey: boolean) => {
    if (placed.has(root)) {
      return
    }
    for (const { name, parent, depth, lanes } of layoutStack(root, name => children.get(name) ?? [])) {
      placed.add(name)
      let showIssueKey = showRootIssueKey
      if (parent !== null) {
        // a stacked branch of another issue than its parent's, unless the branch is named after it
        const issueKey = branches[parent]?.issueKey ?? null
        const childIssueKey = branches[name]?.issueKey ?? null
        showIssueKey = childIssueKey !== null && childIssueKey !== issueKey && childIssueKey !== name
      }
      const dimmed = visible !== null && !visible.has(name)
      // every branch has its node, where it is on its way into master: a table without stacks has one lane
      rows.push(branchRow(name, familyIndent + depth, showIssueKey, familyIndent, hasStacks ? lanes : aloneLanes(lanes), dimmed))
    }
  }
  const placeFamily = (issueKey: string) => {
    for (const member of families.get(issueKey) ?? []) {
      placeStack(member, 1, false)
    }
  }
  for (const name of roots) {
    if (placed.has(name)) {
      continue
    }
    const issueKey = branches[name]?.issueKey ?? null
    if (issueKey === null || issueKey === name) {
      placeStack(name, 0, false)
      if (issueKey === name) {
        placeFamily(issueKey)
      }
      continue
    }
    // placed under the issue branch, wherever that is in the order
    if (branches[issueKey] !== undefined && rootSet.has(issueKey)) {
      continue
    }
    const family = families.get(issueKey)!
    if (family.length === 1) {
      placeStack(name, 0, true)
    } else {
      const at = rows.length
      placeFamily(issueKey)
      // the branches the row heads: its members and the stacks on them
      const members = rows.slice(at).flatMap(row => (row.kind === "branch" ? [row.name] : []))
      rows.splice(at, 0, internRow({ kind: "issue", key: `i:${issueKey}:${members.join(",")}`, issueKey, members }))
    }
  }
  return rows
}

interface RowsCache {
  readonly structure: BranchStructure
  readonly visible: ReadonlySet<string> | null
  readonly rows: BranchTableRow[]
}

// one for all rows, one for the filtered ones: about ten subscribers ask for the rows on every store change; one for the
// Parked card
let allRowsCache: RowsCache | null = null
let visibleRowsCache: RowsCache | null = null
let parkedRowsCache: RowsCache | null = null

/** The rows of the Parked card: the parked branches and those built on one, as a stack graph of their own */
function parkedTableRowsIn(state: BranchSyncState): BranchTableRow[] {
  const structure = structureOf(state)
  if (parkedRowsCache !== null && parkedRowsCache.structure.key === structure.key) {
    return parkedRowsCache.rows
  }
  parkedRowsCache = { structure, visible: null, rows: structure.parkedStates.size === 0 ? [] : computeBranchTableRows(state, null, "parked") }
  return parkedRowsCache.rows
}

/**
 * The rows of the branches in `visible` (all for null) and of the branches they stack on, which are `dimmed` unless
 * visible themselves; computed again only when the structure or `visible` changes.
 */
function branchTableRowsIn(state: BranchSyncState, visible: ReadonlySet<string> | null): BranchTableRow[] {
  const structure = structureOf(state)
  const cache = visible === null ? allRowsCache : visibleRowsCache
  if (cache !== null && cache.visible === visible && cache.structure.key === structure.key) {
    return cache.rows
  }
  const next: RowsCache = { structure, visible, rows: computeBranchTableRows(state, visible) }
  if (visible === null) {
    allRowsCache = next
  } else {
    visibleRowsCache = next
  }
  return next.rows
}

const noBranches: readonly string[] = []

/** The branches stacked on `name`, directly or further down, each after its parent */
export function stackDescendantsIn(state: BranchSyncState, name: string): string[] {
  const { children } = structureOf(state)
  const descendants: string[] = []
  if (children.size === 0) {
    return descendants
  }
  const seen = new Set<string>([name])
  const pending = [name]
  while (pending.length > 0) {
    for (const child of children.get(pending.pop()!) ?? noBranches) {
      if (!seen.has(child)) {
        seen.add(child)
        descendants.push(child)
        pending.push(child)
      }
    }
  }
  return descendants
}

/**
 * The main line of the stack on `root`, root first: the longest chain, lane 0 of the stack graph (`layoutStack`). A Safe
 * Push of its last branch lands the whole line in one robot: the Safe Push of a stack island's header.
 */
export function stackMainLineIn(state: BranchSyncState, root: string): string[] {
  const { children } = structureOf(state)
  return layoutStack(root, name => children.get(name) ?? noBranches)
    .filter(row => row.lanes.lane === 0)
    .map(row => row.name)
}

/** The branches stacked directly on `name`, newest first */
export function getStackChildren(name: string): string[] {
  return (structureOf(branchSyncStore.getState()).children.get(name) ?? noBranches).filter(child => child !== name)
}

/** The branches stacked on `name`, directly or further down */
export function getStackDescendants(name: string): string[] {
  return stackDescendantsIn(branchSyncStore.getState(), name)
}

/** `getStackDescendants` without the parked ones and those parked with them: what lands with the stack of `name` */
export function getUnparkedStackDescendants(name: string): string[] {
  const state = branchSyncStore.getState()
  const { parkedStates } = structureOf(state)
  const descendants = stackDescendantsIn(state, name)
  return parkedStates.size === 0 ? descendants : descendants.filter(it => !parkedStates.has(it))
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

/** The stack parents of the last grouping: child -> parent */
export function useStackParents(): Readonly<Record<string, string>> {
  return useStore(branchSyncStore, state => state.stackParents)
}

/** Whether issue groups put commits into the branch `name` */
export function hasIssueGroup(name: string): boolean {
  return Object.values(getIssueGroups()).includes(name)
}

/** The branch table rows, all of them: newest first, the branches of an issue nested under it, stacks as a graph. */
export function useBranchTableRows(): BranchTableRow[] {
  return useStore(
    branchSyncStore,
    useShallow(state => branchTableRowsIn(state, null)),
  )
}

/**
 * The rows of the `visible` branches (a filter; null shows all) and of the branches they stack on, those dimmed unless
 * visible themselves. The counters of the header and the selection keep `useBranchTableRows`.
 */
export function useVisibleBranchTableRows(visible: ReadonlySet<string> | null): BranchTableRow[] {
  return useStore(
    branchSyncStore,
    useShallow(state => branchTableRowsIn(state, visible)),
  )
}

export function getBranchTableRows(visible: ReadonlySet<string> | null = null): BranchTableRow[] {
  return branchTableRowsIn(branchSyncStore.getState(), visible)
}

/** The rows of the Parked card (`parkedTableRowsIn`); empty without a parked branch */
export function useParkedTableRows(): BranchTableRow[] {
  return useStore(
    branchSyncStore,
    useShallow(state => parkedTableRowsIn(state)),
  )
}

export function getParkedTableRows(): BranchTableRow[] {
  return parkedTableRowsIn(branchSyncStore.getState())
}

/** The branches of the main table, in branch order: every branch but the parked ones and those parked with them */
export function getMainBranchOrder(): readonly string[] {
  return structureOf(branchSyncStore.getState()).mainOrder
}

/** Why the branch stays local: its own line in the parked branches, or a parked branch it is built on; null otherwise */
export function getParkedState(name: string): ParkedState | null {
  return structureOf(branchSyncStore.getState()).parkedStates.get(name) ?? null
}

/** `getParkedState` of the row, re-rendered only when it changes */
export function useParkedState(name: string): ParkedState | null {
  return useStore(branchSyncStore, state => structureOf(state).parkedStates.get(name) ?? null)
}

/** The parked branches of the last grouping: branch -> note, the lines of branches without commits here too */
export function getParkedNotes(): Readonly<Record<string, string>> {
  return branchSyncStore.getState().parked
}

/** The roots of the stacks shown as islands of their own (`stackIslandRoots`), one per line, so a store change notifies only when they change */
function stackIslandKeyIn(state: BranchSyncState): string {
  return [...stackIslandRoots(branchTableRowsIn(state, null), name => state.branches[name]?.commitCount ?? 0)].join("\n")
}

function islandRootsOf(key: string): ReadonlySet<string> {
  return new Set(key === "" ? [] : key.split("\n"))
}

/** The roots of the stacks the main window shows as islands below the branch table (`stackIslandRoots`) */
export function useStackIslandRoots(): ReadonlySet<string> {
  const key = useStore(branchSyncStore, stackIslandKeyIn)
  return useMemo(() => islandRootsOf(key), [key])
}

export function getStackIslandRoots(): ReadonlySet<string> {
  return islandRootsOf(stackIslandKeyIn(branchSyncStore.getState()))
}

/** The branch rows of the table in table order, without the issue rows; of the `visible` branches only (a filter) */
export function getBranchTableOrder(visible: ReadonlySet<string> | null = null): string[] {
  return getBranchTableRows(visible).flatMap(row => (row.kind === "branch" ? [row.name] : []))
}

/** The branch rows of the table and of the Parked card, in their order: what a drag of the selected rows carries */
export function getDraggableBranchOrder(): string[] {
  const state = branchSyncStore.getState()
  return [...branchTableRowsIn(state, null), ...parkedTableRowsIn(state)].flatMap(row => (row.kind === "branch" ? [row.name] : []))
}

function branchCommitsIn(state: BranchSyncState, names: readonly string[]): Commit[] {
  return names.flatMap(name => state.branches[name]?.commits ?? [])
}

/** The commits of `names`, branch by branch */
export function getBranchCommits(names: readonly string[]): Commit[] {
  return branchCommitsIn(branchSyncStore.getState(), names)
}

export function useBranchCommits(names: readonly string[]): Commit[] {
  return useStore(
    branchSyncStore,
    useShallow(state => branchCommitsIn(state, names)),
  )
}

/**
 * The commits with these original hashes, from the branches and the unassigned commits, in the order of `ids`; null
 * when one of them is in no list (a rewrite gave it another id)
 */
export function useFoundCommits(ids: readonly string[]): Commit[] | null {
  return useStore(
    branchSyncStore,
    useShallow(state => (ids.length === 0 ? null : findCommitsIn(state, ids))),
  )
}

/** The commits of `ids` as the last sync left them, in their order; null when one of them is gone */
export function findCommits(ids: readonly string[]): Commit[] | null {
  return findCommitsIn(branchSyncStore.getState(), ids)
}

/**
 * The commits as the last sync left them, in their order; null when one of them is gone. A commit whose id a rewrite
 * changed since is found by what that rewrite kept, when exactly one commit of the table has it: a content rewrite (an
 * amend, a squash, the replay of the commits after them) keeps the author, the author time and the subject without the
 * prefix (`contentKey`); a message rewrite (a move, a reword, an added issue reference) keeps the author and the
 * committer byte for byte, so their times (`messageKey`) find a commit whose subject it changed. A rewrite that waited
 * for another one in the queue acts on the commits the user chose
 */
export function resolveCommits(commits: readonly Commit[]): Commit[] | null {
  const state = branchSyncStore.getState()
  const all = [...Object.values(state.branches).flatMap(branch => branch.commits), ...state.unassignedCommits]
  const byId = new Map(all.map(commit => [commit.originalHash, commit]))
  const byContent = uniqueBy(all, contentKey)
  const byMessage = uniqueBy(all, messageKey)
  const found = commits.map(commit => byId.get(commit.originalHash) ?? byContent.get(contentKey(commit)) ?? byMessage.get(messageKey(commit)) ?? undefined)
  return found.every(commit => commit !== undefined) ? found : null
}

/**
 * The commits a rewrite of `repositoryPath` acts on when its turn comes: as the last sync left them (`resolveCommits`)
 * while the table shows that repository, so a rewrite that waited in the queue sends the ids the rewrites before it
 * gave them; else, or when one of them is gone, by the ids they had
 */
export function resolveCommitsOf<T extends Commit>(repositoryPath: string, commits: readonly T[]): readonly Commit[] {
  return (isSelectedRepository(repositoryPath) ? resolveCommits(commits) : null) ?? commits
}

/** The commits by `key`; null for a key that two commits share, which finds none */
function uniqueBy(commits: readonly Commit[], key: (commit: Commit) => string): Map<string, Commit | null> {
  const result = new Map<string, Commit | null>()
  for (const commit of commits) {
    const value = key(commit)
    result.set(value, result.has(value) ? null : commit)
  }
  return result
}

/** What a content rewrite keeps of a commit: its author, the author time and the subject without the prefix */
function contentKey(commit: Commit): string {
  return `${commit.author}\n${commit.authorTime}\n${commit.strippedSubject}`
}

/** What a message rewrite keeps of a commit, its subject aside: its author, the author time and the committer time */
function messageKey(commit: Commit): string {
  return `${commit.author}\n${commit.authorTime}\n${commit.committerTime}`
}

function findCommitsIn({ branches, unassignedCommits }: BranchSyncState, ids: readonly string[]): Commit[] | null {
  const byId = new Map<string, Commit>()
  for (const branch of Object.values(branches)) {
    for (const commit of branch.commits) {
      byId.set(commit.originalHash, commit)
    }
  }
  for (const commit of unassignedCommits) {
    byId.set(commit.originalHash, commit)
  }
  const found = ids.map(id => byId.get(id))
  return found.every(commit => commit !== undefined) ? found : null
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
  const key = pushKey(useSelectedProjectPath() ?? "", name)
  return useStore(branchSyncStore, state => Object.hasOwn(state.pushingBranches, key))
}

/**
 * The branches a stack push started from the branch sends, parents first (`stackPushes`; empty while it waits for a
 * sync); null when none runs
 */
export function useStackPush(startedFrom: string): readonly string[] | null {
  const key = pushKey(useSelectedProjectPath() ?? "", startedFrom)
  return useStore(branchSyncStore, state => state.stackPushes[key] ?? null)
}

/**
 * Whether the stack branch of the whole stack of `root` is being pushed: alone (`pushWholeStack`) or after the branches
 * of a stack push that takes it along (`pushWithChildren`)
 */
export function useIsStackBranchPushing(root: string): boolean {
  return useIsBranchPushing(stackReadinessKey(root))
}

/** How far the running push of the stack branch of the whole stack of `root` is (`useBranchPushProgress`) */
export function useStackBranchPushProgress(root: string): PushProgress | null {
  return useBranchPushProgress(stackReadinessKey(root))
}

/**
 * How far the running push of the branch in the selected repository is; null before git reports a stage and when no
 * push runs
 */
export function useBranchPushProgress(name: string): PushProgress | null {
  const key = pushKey(useSelectedProjectPath() ?? "", name)
  return useStore(pushProgressStore, state => state[key] ?? null)
}

export function getBranchPushProgress(name: string, repositoryPath: string = selectedRepositoryPath()): PushProgress | null {
  return pushProgressStore.getState()[pushKey(repositoryPath, name)] ?? null
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
  const repositoryPath = useSelectedProjectPath()
  return useStore(branchSyncStore, state => (state.baselineStatus?.repositoryPath === repositoryPath ? state.baselineStatus : null))
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

/** The names of the branches that hold the commits (by their original ids), as the last sync knows them */
export function getBranchesOfCommits(originalIds: readonly string[]): string[] {
  return Object.values(branchSyncStore.getState().branches)
    .filter(branch => originalIds.some(id => branch.commitIndex[id] !== undefined))
    .map(branch => branch.name)
}

/** The stack branch of the stack rooted at `root`; undefined for a stack that doesn't fork or before a sync reported it */
export function useStackBranch(root: string): StackBranch | undefined {
  return useStore(branchSyncStore, state => state.stackBranches[root])
}

export function getStackBranch(root: string): StackBranch | undefined {
  return branchSyncStore.getState().stackBranches[root]
}

/**
 * The root whose whole stack a Safe Push of `name` can land instead (the popover offers it): the root of the stack
 * that forks and holds the branch (`StackBranch.members`), else `name` itself while it heads a straight chain; null for
 * a branch outside a stack and for one inside a straight chain, whose whole stack its last branch's Safe Push lands
 */
export function wholeStackRootIn(state: BranchSyncState, name: string): string | null {
  const forking = Object.values(state.stackBranches).find(stack => stack.members.includes(name))
  if (forking !== undefined) {
    return forking.root
  }
  return stackParentIn(state.branches, name) === null && stackDescendantsIn(state, name).length > 0 ? name : null
}

/** The stack branch of the stack that forks and holds the branch `name` (`StackBranch.members`); undefined otherwise */
export function getStackBranchOf(name: string): StackBranch | undefined {
  return Object.values(branchSyncStore.getState().stackBranches).find(stack => stack.members.includes(name))
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
  return useStore(
    branchSyncStore,
    useShallow(state => moveTargetsIn(state, sourceBranch, limit)),
  )
}

export function getMoveTargets(sourceBranch: string | null, limit: number): string[] {
  return moveTargetsIn(branchSyncStore.getState(), sourceBranch, limit)
}

// ---- branch updates ----

/** Immutable update of a single branch; no-op if the branch is unknown or the updater returns the same record. */
function updateBranch(name: string, updater: (branch: Branch) => Branch): void {
  branchSyncStore.setState(state => {
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
  updateBranch(name, branch => (branch.autoExpandRequested || branch.autoScrollRequested ? { ...branch, autoExpandRequested: false, autoScrollRequested: false } : branch))
}

/** The key of a push in flight (`pushingBranches`): the repository it started in and the branch */
function pushKey(repositoryPath: string, name: string): string {
  return `${repositoryPath}\n${name}`
}

function selectedRepositoryPath(): string {
  return getSelectedProject()?.path ?? ""
}

/** Whether a push of the branch runs in the repository (by default the selected one) */
export function isBranchPushing(name: string, repositoryPath: string = selectedRepositoryPath()): boolean {
  return Object.hasOwn(branchSyncStore.getState().pushingBranches, pushKey(repositoryPath, name))
}

/** Resolves once no push of the branch runs in the repository (the selected one by default): at once while none does */
export function whenBranchPushed(name: string, repositoryPath: string = selectedRepositoryPath()): Promise<void> {
  const key = pushKey(repositoryPath, name)
  if (!Object.hasOwn(branchSyncStore.getState().pushingBranches, key)) {
    return Promise.resolve()
  }
  return new Promise(resolve => {
    const unsubscribe = branchSyncStore.subscribe(state => {
      if (!Object.hasOwn(state.pushingBranches, key)) {
        unsubscribe()
        resolve()
      }
    })
  })
}

/**
 * Marks the push of the branch in `repositoryPath`, the repository it started in, as running or ended: the rows of a
 * branch of the same name in another repository show no push of it
 */
export function setBranchPushing(name: string, isPushing: boolean, repositoryPath: string): void {
  const key = pushKey(repositoryPath, name)
  branchSyncStore.setState(state => {
    if (Object.hasOwn(state.pushingBranches, key) === isPushing) {
      return state
    }
    if (isPushing) {
      return { pushingBranches: { ...state.pushingBranches, [key]: true } }
    }
    const { [key]: _, ...pushingBranches } = state.pushingBranches
    return { pushingBranches }
  })
  // also after a switch of the repository: the next push of the branch starts without this one's percent
  if (!isPushing && Object.hasOwn(pushProgressStore.getState(), key)) {
    const { [key]: _, ...progress } = pushProgressStore.getState()
    pushProgressStore.setState(progress, true)
  }
}

/**
 * By `pushKey`: the pushes that wait (for a sync, a rewrite) with the spinner of a push and send nothing yet. A push
 * reads its branch once the wait ends, so the remote status a sync computes meanwhile is the one it must push over
 */
const waitingPushes = new Set<string>()

/**
 * Marks a push of the branch in `repositoryPath` as waiting before it sends, or the wait as ended: the row shows the
 * push (`isBranchPushing`, and a stack push waits for it), and the remote status of a sync meanwhile still reaches the
 * row, unlike the one a sync computes while the push sends
 */
export function setBranchPushWaiting(name: string, waiting: boolean, repositoryPath: string): void {
  const key = pushKey(repositoryPath, name)
  if (waiting) {
    waitingPushes.add(key)
  } else {
    waitingPushes.delete(key)
  }
  setBranchPushing(name, waiting, repositoryPath)
}

/** Whether a push of the branch in the repository sends it now: a push that only waits does not */
function isBranchSending(name: string, repositoryPath: string): boolean {
  return isBranchPushing(name, repositoryPath) && !waitingPushes.has(pushKey(repositoryPath, name))
}

/** Whether a stack push started from `startedFrom` runs in the repository (by default the selected one) */
export function isStackPushing(startedFrom: string, repositoryPath: string = selectedRepositoryPath()): boolean {
  return Object.hasOwn(branchSyncStore.getState().stackPushes, pushKey(repositoryPath, startedFrom))
}

/**
 * The branches a stack push started from `startedFrom` in `repositoryPath`, the repository it started in, sends
 * (`stackPushes`); null when it ended. The row of the same name in another repository shows no push of it and pushes
 * its own stack.
 */
export function setStackPush(startedFrom: string, names: readonly string[] | null, repositoryPath: string): void {
  const key = pushKey(repositoryPath, startedFrom)
  branchSyncStore.setState(state => {
    if (names === null) {
      if (!Object.hasOwn(state.stackPushes, key)) {
        return state
      }
      const { [key]: _, ...stackPushes } = state.stackPushes
      return { stackPushes }
    }
    return { stackPushes: { ...state.stackPushes, [key]: names } }
  })
}

/**
 * Records how far the push of the branch in `repositoryPath` is, for its rows there; a message that arrives after the
 * push ended is dropped
 */
export function setBranchPushProgress(name: string, progress: PushProgress, repositoryPath: string): void {
  setBranchesPushProgress([name], progress, repositoryPath)
}

/** Records how far the one push of the branches is, for each of them in one update; the ones whose push ended drop it */
export function setBranchesPushProgress(names: readonly string[], progress: PushProgress, repositoryPath: string): void {
  const pushing = names.filter(name => isBranchPushing(name, repositoryPath))
  if (pushing.length > 0) {
    pushProgressStore.setState(Object.fromEntries(pushing.map(name => [pushKey(repositoryPath, name), progress])))
  }
}

/**
 * The remote status of the branch after a push of it in `repositoryPath`; nothing once another repository is selected:
 * its branch of the same name is another branch
 */
export function setBranchRemoteStatus(name: string, remoteStatus: RemoteStatus, repositoryPath: string): void {
  if (repositoryPath === selectedRepositoryPath()) {
    updateBranch(name, branch => withRemoteStatus(branch, remoteStatus))
  }
}

/**
 * The remote status of the stack branch of `root` after a push of it in `repositoryPath` (`pushWholeStack`), until the
 * next sync reports it again; nothing for a stack branch the store doesn't have, nor once another repository is selected
 */
export function setStackBranchRemoteStatus(root: string, remoteStatus: RemoteStatus, repositoryPath: string): void {
  const stack = branchSyncStore.getState().stackBranches[root]
  if (repositoryPath !== selectedRepositoryPath() || stack === undefined || (stack.remoteStatus !== null && sameRemoteStatus(stack.remoteStatus, remoteStatus))) {
    return
  }
  branchSyncStore.setState(state => ({ stackBranches: { ...state.stackBranches, [root]: { ...stack, remoteStatus } } }))
}

/** The branch with `remoteStatus`; the same record when its status has the same fields, so no subscriber re-renders */
function withRemoteStatus(branch: Branch, remoteStatus: RemoteStatus): Branch {
  return branch.remoteStatus !== null && sameRemoteStatus(branch.remoteStatus, remoteStatus) ? branch : { ...branch, remoteStatus }
}

function sameRemoteStatus(a: RemoteStatus, b: RemoteStatus): boolean {
  return (
    a.exists === b.exists &&
    a.commitsAhead === b.commitsAhead &&
    a.commitsBehind === b.commitsBehind &&
    a.commitsAmended === b.commitsAmended &&
    a.commitsRestacked === b.commitsRestacked &&
    a.commitsLanded === b.commitsLanded &&
    a.commitsReworded === b.commitsReworded &&
    a.ancestorCommitsAhead === b.ancestorCommitsAhead &&
    a.myCommitsAhead === b.myCommitsAhead &&
    a.fastForward === b.fastForward &&
    a.lastPushTime === b.lastPushTime &&
    a.remoteTip === b.remoteTip &&
    sameItems(a.unpushedCommits, b.unpushedCommits) &&
    a.droppedCommits.length === b.droppedCommits.length &&
    a.droppedCommits.every((commit, index) => sameDroppedCommit(commit, b.droppedCommits[index]!))
  )
}

function sameDroppedCommit(a: DroppedCommit, b: DroppedCommit): boolean {
  return a.id === b.id && a.subject === b.subject && a.author === b.author && a.authorTime === b.authorTime
}

function sameItems(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((item, index) => item === b[index])
}

type RemoteStatusFields = Pick<
  RemoteStatusUpdate,
  | "remoteExists"
  | "unpushedCommits"
  | "commitsBehind"
  | "commitsAmended"
  | "commitsRestacked"
  | "commitsLanded"
  | "commitsReworded"
  | "ancestorCommitsAhead"
  | "myUnpushedCount"
  | "fastForward"
  | "lastPushTime"
  | "remoteTip"
  | "droppedCommits"
>

export function toRemoteStatus(update: RemoteStatusFields): RemoteStatus {
  return {
    exists: update.remoteExists,
    unpushedCommits: update.unpushedCommits,
    commitsAhead: update.unpushedCommits.length,
    commitsBehind: update.commitsBehind,
    commitsAmended: update.commitsAmended,
    commitsRestacked: update.commitsRestacked,
    commitsLanded: update.commitsLanded,
    commitsReworded: update.commitsReworded,
    ancestorCommitsAhead: update.ancestorCommitsAhead,
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

/**
 * The run whose events the store applies. Runs are serialized, yet a channel can deliver a message after its invoke
 * resolved, and a reset (another repository) makes a run stale: the events of an older run are dropped by it.
 */
let syncGeneration = 0
/** The running chain of syncs, one sync at a time per window; null while none runs */
let syncChain: Promise<void> | null = null
/** A change asked for a sync while one ran: the chain syncs once more after it */
let followUpRequested = false
/** A focus or ⌘R came while a sync ran: once the chain ends, `syncIfChanged` checks what changed meanwhile */
let recheckAfterChain = false

/** A request of `requestSync`: told when the run that serves it starts and when it ends */
interface RunRequest {
  readonly started: (run: number) => void
  readonly ended: () => void
}

/** The requests the next run serves: the first run of a chain, or the follow-up of a request made while one ran */
let runRequests: RunRequest[] = []

function endRunRequests(requests: readonly RunRequest[]): void {
  for (const request of requests) {
    request.ended()
  }
}

/** Every request while a sync runs is the resync of a change (a rewrite, its Undo, a fetch): quiet, as they ask */
const FOLLOW_UP_OPTIONS: SyncOptions = { autoScroll: false, autoExpand: false }

/** What a sync names: the selected repository; Rust resolves the branch prefix of each command itself */
interface SyncRequest {
  readonly repositoryPath: string
}

/** The sync request of the selected repository; @throws UserError while none is selected */
function syncRequest(): SyncRequest {
  const repositoryPath = getSelectedRepositoryPath()
  if (repositoryPath === "") {
    throw new UserError("Please enter a repository path")
  }
  return { repositoryPath }
}

/**
 * The fingerprint of what a sync of the repository reads under the prefix Rust resolves (a changed prefix changes it);
 * null when git can't tell (the sync then runs anyway)
 */
async function readSyncInputs(request: SyncRequest): Promise<string | null> {
  try {
    const result = await commands.getSyncInputsFingerprint({ repositoryPath: request.repositoryPath })
    return result.status === "ok" ? result.data : null
  } catch {
    return null
  }
}

/** Whether the last successful sync read these inputs of the repository it would sync now */
function isSyncedWith(request: SyncRequest, fingerprint: string | null): boolean {
  const state = branchSyncStore.getState()
  const known = state.syncedInputs
  return known !== null && fingerprint === known.fingerprint && known.repositoryPath === request.repositoryPath && state.syncedRepositoryPath === request.repositoryPath
}

/**
 * Auto-sync on Focus: syncs only when what a sync reads changed since the last one (a commit, a fetch or a push
 * elsewhere, a regroup by `bd`, a description), one cheap read otherwise; a sync the user didn't ask for, so a deferred
 * one (a rebase in the IDE) waits silently. While a sync runs, it checks once after it: a commit in the IDE during a
 * long sync that started before it is not lost, and a sync that saw it costs no second one.
 */
export async function syncIfChanged(): Promise<void> {
  if (branchSyncStore.getState().isSyncing) {
    recheckAfterChain = true
    return
  }
  const request = syncRequest()
  const known = branchSyncStore.getState().syncedInputs
  if (known !== null && isSyncedWith(request, known.fingerprint)) {
    const inputs = await readSyncInputs(request)
    // a sync started during the read reads the inputs after it; one that ended during it is what `isSyncedWith` checks
    if (branchSyncStore.getState().isSyncing) {
      return
    }
    if (isSyncedWith(request, inputs)) {
      branchSyncStore.setState({ inputsCheckedAt: Date.now() })
      return
    }
  }
  await syncBranches()
}

/** Resolves once no sync runs: at once while none does, else at the end of the chain (its follow-up included) */
export function whenSynced(): Promise<void> {
  if (!branchSyncStore.getState().isSyncing) {
    return Promise.resolve()
  }
  return new Promise(resolve => {
    const unsubscribe = branchSyncStore.subscribe(state => {
      if (!state.isSyncing) {
        unsubscribe()
        resolve()
      }
    })
  })
}

/**
 * Syncs the selected repository, one sync at a time: a request while one runs asks for one quiet follow-up after it,
 * which reads what changed meanwhile; all requests resolve when the chain ends. `isSyncing` stays true across the
 * chain, so a push waiting for it (`whenSynced`) never pushes refs a sync still rewrites, and the events of two runs
 * never mix.
 */
export function syncBranches(options?: SyncOptions): Promise<void> {
  if (syncChain !== null) {
    followUpRequested = true
    return syncChain
  }
  branchSyncStore.setState({ isSyncing: true })
  syncChain = runSyncChain(options)
  return syncChain
}

/**
 * `syncBranches`, but resolves when the run that serves this request ends, not the chain: the run that starts now, or the
 * follow-up of a sync that runs already. `onStart` gets the run (`groupedRun` tells when the branches are its own). An
 * update waits for its own sync this way: a ⌘R during it asks for a follow-up that belongs to Sync only.
 */
export function requestSync(options: SyncOptions | undefined, onStart: (run: number) => void = () => {}): Promise<void> {
  return new Promise(resolve => {
    runRequests.push({ started: onStart, ended: resolve })
    void syncBranches(options)
  })
}

/**
 * How far the sync run is, 0 to 1: its synced commits of all the commits it groups; null before its grouping arrived,
 * when the branches are still those of an older run, or without commits
 */
export function selectSyncFraction(state: BranchSyncState, run: number): number | null {
  if (state.groupedRun !== run) {
    return null
  }
  let processed = 0
  let total = 0
  for (const name of state.branchOrder) {
    const branch = state.branches[name]
    if (branch !== undefined) {
      processed += Math.min(branch.processedCount, branch.commitCount)
      total += branch.commitCount
    }
  }
  return total === 0 ? null : processed / total
}

async function runSyncChain(options: SyncOptions | undefined): Promise<void> {
  let repositoryPath: string | null
  try {
    repositoryPath = await runSync(options)
    // another repository selected meanwhile has nothing to resync for these changes: its open syncs it
    while (followUpRequested && (repositoryPath === null || repositoryPath === getSelectedRepositoryPath())) {
      followUpRequested = false
      repositoryPath = await runSync(FOLLOW_UP_OPTIONS)
    }
  } finally {
    syncChain = null
    followUpRequested = false
    branchSyncStore.setState({ isSyncing: false })
    // a follow-up skipped for another repository: its requests have no run
    const left = runRequests
    runRequests = []
    endRunRequests(left)
  }
  if (recheckAfterChain) {
    recheckAfterChain = false
    if (repositoryPath === getSelectedRepositoryPath()) {
      void syncIfChanged()
    }
  }
}

/** One sync run, streaming its events into the store; never rejects. Returns the repository it synced, null if none. */
async function runSync(options: SyncOptions | undefined): Promise<string | null> {
  // the outcome of the last run is what stays
  branchSyncStore.setState({ syncError: null })
  const generation = ++syncGeneration
  const startedAt = Date.now()
  const served = runRequests
  runRequests = []
  for (const request of served) {
    request.started(generation)
  }
  let repositoryPath: string | null = null

  try {
    // the prefix is Rust's: the sync resolves it from the git config it reads, so nothing waits for the page's
    const target = syncRequest()
    repositoryPath = target.repositoryPath
    // before the sync: a commit made while it runs is a change the next focus syncs
    const inputs = await readSyncInputs(target)
    const channel = new Channel<SyncEvent>()
    channel.onmessage = event => {
      if (generation === syncGeneration) {
        queueSyncEvent(event, target.repositoryPath, options)
      }
    }

    const result = await commands.syncBranches(target, channel)
    flushSyncEvents()
    if (generation !== syncGeneration) {
      // the branches of another repository replaced these meanwhile
      return repositoryPath
    }
    if (result.status === "error") {
      const code = result.error.code
      if (code === "operation_in_progress" || code === "detached_head") {
        // a precondition, not a failure: the branches stay as the last sync left them
        if (options?.manual === true) {
          showToast({
            type: "info",
            title: result.error.message,
            description: code === "operation_in_progress" ? "The branches sync once git finished it" : "Check out a branch to sync its commits",
          })
        }
        return repositoryPath
      }
      branchSyncStore.setState({ syncError: result.error.message || "Failed to sync branches" })
      console.error("Sync error:", result.error)
      return repositoryPath
    }

    // the repository of the sync, also when another one was selected since
    // the usual time of a sync is the share of the sync of an update (`update-progress.ts`): every sync teaches it
    const syncedAt = Date.now()
    updateProject(target.repositoryPath, project => ({
      ...project,
      lastSyncTime: syncedAt,
      lastBranchCount: branchSyncStore.getState().branchOrder.length,
      updateTimings: addTiming(project.updateTimings, "sync", syncedAt - startedAt),
    }))
    branchSyncStore.setState({
      syncedRepositoryPath: target.repositoryPath,
      syncedInputs: inputs === null ? null : { ...target, fingerprint: inputs },
      inputsCheckedAt: null,
    })
    hideAfterSync(target.repositoryPath)
    void loadAutosquashPlan(target.repositoryPath)
  } catch (error) {
    branchSyncStore.setState({ syncError: (error instanceof Error ? error.message : String(error)) || "Failed to sync branches" })
    console.error("Sync error:", error)
  } finally {
    // the events of a failed sync too
    flushSyncEvents()
    // a placeholder still waiting got no result, also after a failed sync; a newer sync (the follow-up) reports its own
    if (generation === syncGeneration && !followUpRequested) {
      settleArchivedPlaceholders()
    }
    branchSyncStore.setState(state => ({ syncCount: state.syncCount + 1 }))
    endRunRequests(served)
  }
  return repositoryPath
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

/**
 * Forgets the branches of the previous repository when the window opens another one, so they never show under its
 * name. Pushes in flight keep their state: they finish in their own repository.
 */
export function resetBranchSync(): void {
  autosquashPlanRequest++
  // the events of a sync of the previous repository not applied yet, and those still on their way
  syncGeneration++
  queuedSyncEvents = []
  if (flushTimer !== null) {
    clearTimeout(flushTimer)
    flushTimer = null
  }
  branchSyncStore.setState({
    syncError: null,
    syncedRepositoryPath: null,
    baselineBranch: null,
    branchOrder: [],
    branches: {},
    unassignedCommits: [],
    autosquashPlan: null,
    baselineStatus: null,
    stacksEnabled: false,
    issueGroups: {},
    stackParents: {},
    parked: {},
    stackBranches: {},
  })
  resetArchivedBranches()
  resetUnappliedBranches()
}

// ---- sync events ----

/**
 * How long sync events gather before one store update applies them, about a frame. The backend sends an event per
 * commit; applied one by one, each would re-run every selector of the store (docs/design/rendering-performance.md).
 */
const SYNC_EVENT_FLUSH_MS = 16

interface QueuedSyncEvent {
  readonly event: SyncEvent
  readonly repositoryPath: string
  readonly options: SyncOptions | undefined
}

let queuedSyncEvents: QueuedSyncEvent[] = []
let flushTimer: ReturnType<typeof setTimeout> | null = null

function queueSyncEvent(event: SyncEvent, repositoryPath: string, options: SyncOptions | undefined): void {
  // decided on arrival, not at the flush: the push result is authoritative, and a push that ends before the flush
  // would let a status the sync computed mid-push overwrite it. A push that only waits for this sync sends after it,
  // over the status it brings: a fetch moved the remote branch, and the push goes over the new tip at once
  if (event.type === "remoteStatusUpdate" && isBranchSending(event.data.branchName, repositoryPath)) {
    return
  }
  queuedSyncEvents.push({ event, repositoryPath, options })
  flushTimer ??= setTimeout(flushSyncEvents, SYNC_EVENT_FLUSH_MS)
}

/**
 * The store fields a batch of sync events changes, committed in one update. `patch.branches` is copied from the store
 * on the first change of a branch, then written in place.
 */
interface SyncDraft {
  readonly patch: Partial<BranchSyncState>
  ownsBranches: boolean
  /** The integration info of archived branches, applied in one update of their store */
  integrationInfos: BranchIntegrationInfo[]
}

/**
 * Applies the queued sync events in one store update, in event order; the updates of other stores (the project, the
 * archived and landed branches) run during it, before the commit.
 */
function flushSyncEvents(): void {
  if (flushTimer !== null) {
    clearTimeout(flushTimer)
    flushTimer = null
  }
  const events = queuedSyncEvents
  if (events.length === 0) {
    return
  }
  queuedSyncEvents = []
  const draft: SyncDraft = { patch: {}, ownsBranches: false, integrationInfos: [] }
  for (const { event, repositoryPath, options } of events) {
    applySyncEvent(draft, event, repositoryPath, options)
  }
  applyIntegrationInfos(draft)
  if (Object.keys(draft.patch).length > 0) {
    branchSyncStore.setState(draft.patch)
  }
}

function applyIntegrationInfos(draft: SyncDraft): void {
  if (draft.integrationInfos.length > 0) {
    updateFromIntegrationInfos(draft.integrationInfos)
    draft.integrationInfos = []
  }
}

function draftBranches(draft: SyncDraft): Record<string, Branch> {
  return draft.patch.branches ?? branchSyncStore.getState().branches
}

/** Updates one branch of the draft; no-op if the branch is unknown or the updater returns the same record */
function updateDraftBranch(draft: SyncDraft, name: string, updater: (branch: Branch) => Branch): void {
  const branches = draftBranches(draft)
  const branch = branches[name]
  if (branch === undefined) {
    return
  }
  const next = updater(branch)
  if (next === branch) {
    return
  }
  if (!draft.ownsBranches) {
    draft.patch.branches = { ...branches }
    draft.ownsBranches = true
  }
  draft.patch.branches![name] = next
}

function applySyncEvent(draft: SyncDraft, event: SyncEvent, repositoryPath: string, options?: SyncOptions): void {
  switch (event.type) {
    case "issueNavigationConfig":
      // the repository of the sync, also when another one was selected since
      if (event.data.config) {
        updateProject(repositoryPath, { issueNavigationConfig: event.data.config })
      }
      break
    case "branchesGrouped":
      applyBranchesGrouped(draft, event.data)
      break
    case "commitSynced":
      applyCommitSynced(draft, event.data)
      break
    case "commitError":
      applyCommitError(draft, event.data, options)
      break
    case "branchStatusUpdate":
      applyBranchStatusUpdate(draft, event.data, options)
      break
    case "unassignedCommits":
      draft.patch.unassignedCommits = event.data.commits
      break
    case "archivedBranchesFound":
      // the names reconcile what the infos before them added
      applyIntegrationInfos(draft)
      updateFromArchivedNames(event.data.branchNames, event.data.newlyArchived)
      break
    case "unappliedBranchesFound":
      setUnappliedBranches(event.data.branches.map(({ remote, ...info }) => ({ ...info, remoteStatus: remote === null ? null : toRemoteStatus(remote) })))
      break
    case "branchIntegrationDetected":
      draft.integrationInfos.push(event.data.info)
      break
    case "remoteStatusUpdate": {
      // one that arrived while the branch was pushing was dropped by `queueSyncEvent`
      const remoteStatus = toRemoteStatus(event.data)
      updateDraftBranch(draft, event.data.branchName, branch => withRemoteStatus(branch, remoteStatus))
      break
    }
    case "stackBranchUpdate":
      applyStackBranchUpdate(draft, event.data)
      break
    case "baselineStatus": {
      const { landing, landed, ...status } = event.data
      draft.patch.baselineStatus = { repositoryPath, ...status }
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
    descriptionTitle: info.descriptionTitle ?? null,
    remoteStatus,
    allCommitsHaveIssueReferences: info.allCommitsHaveIssueReferences,
    myEmail: info.myEmail ?? null,
    origin: info.origin,
    issueKey: info.issueKey ?? null,
    stack: info.stack ?? null,
  }
}

/**
 * A branch the sync groups as it was, with the same commits, which the last sync built without a problem: its row keeps
 * what it shows while it is rebuilt, so a sync of 35 branches doesn't turn each row to "syncing…" for a moment
 */
function isQuietResync(previous: Branch, info: GroupedBranchInfo): boolean {
  if (
    previous.hasError ||
    previous.status === "Syncing" ||
    previous.status === "Error" ||
    previous.status === "MergeConflict" ||
    previous.status === "AnalyzingConflict" ||
    previous.status === "Blocked"
  ) {
    return false
  }
  return previous.commits.length === info.commits.length && info.commits.every((commit, index) => previous.commits[index]!.originalHash === commit.originalHash)
}

/** The record of a quiet resync: the grouping of now, the status and the synced hashes of the last sync until this one reports */
function keepShownState(previous: Branch, info: GroupedBranchInfo): Branch {
  return {
    ...createBranchFromGroup(info, previous.remoteStatus),
    status: previous.status,
    statusText: previous.statusText,
    commits: info.commits.map((commit, index) => {
      const shown = previous.commits[index]!
      return { ...commit, hash: shown.hash, status: shown.status }
    }),
  }
}

/**
 * Reconciles the branch list with the grouping result: backend order; a changed or new branch (re)starts in the syncing
 * state (an existing one is reset, which a fresh record keyed by the same name does), an unchanged one keeps what its
 * row shows (`isQuietResync`); branches no longer present are dropped. The last known remote status is kept until the
 * sync reports a fresh one, so the push button label does not flicker.
 */
function applyBranchesGrouped(draft: SyncDraft, data: Extract<SyncEvent, { type: "branchesGrouped" }>["data"]): void {
  // only the events of the current run are applied (`syncGeneration`)
  draft.patch.groupedRun = syncGeneration
  const previous = draftBranches(draft)
  const branches: Record<string, Branch> = {}
  const branchOrder: string[] = []
  for (const info of data.branches) {
    if (info.name in branches) {
      continue
    }
    const shown = previous[info.name]
    branches[info.name] = shown !== undefined && isQuietResync(shown, info) ? keepShownState(shown, info) : createBranchFromGroup(info, shown?.remoteStatus ?? null)
    branchOrder.push(info.name)
  }
  const issueGroups = data.issueGroups ?? {}
  // the same groups keep their record: a sync rarely changes them
  const previousGroups = draft.patch.issueGroups ?? branchSyncStore.getState().issueGroups
  const stackParents = data.stackParents ?? {}
  const previousStackParents = draft.patch.stackParents ?? branchSyncStore.getState().stackParents
  const parked = data.parked ?? {}
  const previousParked = draft.patch.parked ?? branchSyncStore.getState().parked
  Object.assign(draft.patch, {
    baselineBranch: data.baselineBranch,
    branches,
    branchOrder,
    stacksEnabled: data.stacks ?? false,
    issueGroups: sameStringRecord(issueGroups, previousGroups) ? previousGroups : issueGroups,
    stackParents: sameStringRecord(stackParents, previousStackParents) ? previousStackParents : stackParents,
    parked: sameStringRecord(parked, previousParked) ? previousParked : parked,
    stackBranches: announcedStackBranches(draft.patch.stackBranches ?? branchSyncStore.getState().stackBranches, data.stackBranches ?? []),
  })
  draft.ownsBranches = true
}

/** The stack branches a grouping announces, as the last sync left them until their update; the same record when it has them all */
function announcedStackBranches(previous: Readonly<Record<string, StackBranch>>, roots: readonly string[]): Readonly<Record<string, StackBranch>> {
  const kept: Record<string, StackBranch> = {}
  for (const root of roots) {
    const stackBranch = previous[root]
    if (stackBranch !== undefined) {
      kept[root] = stackBranch
    }
  }
  return Object.keys(kept).length === Object.keys(previous).length ? previous : kept
}

function toStackBranch(update: StackBranchUpdate): StackBranch {
  return {
    root: update.root,
    branchName: update.branchName,
    members: update.members,
    commitCount: update.commitCount,
    state: update.state,
    tip: update.tip,
    blockedBy: update.blockedBy,
    conflict: update.conflict,
    error: update.error,
    remoteStatus: update.remote === null ? null : toRemoteStatus(update.remote),
  }
}

function applyStackBranchUpdate(draft: SyncDraft, update: StackBranchUpdate): void {
  const stackBranches = draft.patch.stackBranches ?? branchSyncStore.getState().stackBranches
  draft.patch.stackBranches = { ...stackBranches, [update.root]: toStackBranch(update) }
}

function applyCommitSynced(draft: SyncDraft, data: Extract<SyncEvent, { type: "commitSynced" }>["data"]): void {
  const { branchName, commitHash, newHash, status, resolved } = data
  updateDraftBranch(draft, branchName, branch => {
    if (branch.commitIndex[commitHash] === undefined) {
      return branch
    }
    return {
      ...branch,
      commits: updateCommit(branch, commitHash, { hash: newHash, status, resolved: resolved ?? null }),
      processedCount: branch.processedCount + 1,
    }
  })
}

function applyCommitError(draft: SyncDraft, data: Extract<SyncEvent, { type: "commitError" }>["data"], options?: SyncOptions): void {
  const { branchName, commitHash, error } = data
  const autoExpand = options?.autoExpand ?? true
  updateDraftBranch(draft, branchName, branch => {
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

function applyBranchStatusUpdate(draft: SyncDraft, data: Extract<SyncEvent, { type: "branchStatusUpdate" }>["data"], options?: SyncOptions): void {
  const { branchName, status, error } = data
  updateDraftBranch(draft, branchName, branch => {
    if (status === "Blocked") {
      // not an error of this branch: its parent can't be built on; the reason is the tooltip
      return { ...branch, status, statusText: error?.Generic ?? "blocked" }
    }
    // a stack problem names itself as Rust says it ("conflicting parents"), not as a failure
    const problem = status === "Error" ? branch.stack?.problems[0] : undefined
    const next: MutableBranch = { ...branch, status, statusText: problem?.label ?? getStatusText(status) }
    // the commits whose prefix the problem asks to reword carry the error mark, the others stay as they are
    for (const hash of problem?.commits ?? []) {
      next.commits = updateCommit(next, hash, { status: "Error" })
    }
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
