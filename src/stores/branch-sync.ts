import { useMemo } from "react"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { createStore } from "zustand/vanilla"
import type {
  AutosquashPlan,
  BranchError,
  BranchOrigin,
  BranchStack,
  BranchStatus,
  Commit,
  CommitSyncStatus,
  DroppedCommit,
  PushAction,
  RemoteStatusUpdate,
  StackBranchConflict,
  StackBranchState,
  StackMemberBlock,
  SyncReason,
  SyncStep,
  UpdateCheck,
  Version,
  WordResolutionSummary,
} from "@/lib/bindings"
import type { ParkedState } from "@/lib/parked"
import { collectParkedStates } from "@/lib/parked"
import { stackIslandRoots } from "@/lib/stack-collapse"
import type { StackLanes } from "@/lib/stack-lanes"
import { layoutStack } from "@/lib/stack-lanes"
import type { SyncDeferral } from "@/lib/sync-deferred"
import type { PushProgress } from "@/lib/transfer-progress"
import { stackReadinessKey } from "@/lib/whole-stack"
import { nextRunNumber, requestSessionSync } from "@/model/store"
import { getSelectedProject, isSelectedRepository, useSelectedProjectPath } from "@/stores/repository"

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
  /** What a push of the branch does over this status, as Rust decides it (`domain_policy::push::action`) */
  action: PushAction
}

/**
 * Virtual branch, built from the parts of its branch in the view of the session (`src/model/store.ts`).
 * Immutable: a patch that changes a part of it creates a new record, so a component selecting one branch re-renders only
 * when that branch changes.
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
  /** A sync runs in the session, or a request of this window for one has not answered yet (`syncBranches`) */
  isSyncing: boolean
  /**
   * Finished sync runs of the session this window saw, failed ones included: the sync button exposes it so a test can
   * wait for the next sync. A sync and its follow-up count two, `isSyncing` stays true between them.
   */
  syncCount: number
  syncError: string | null
  /** The last sync git refused before it built anything (a rebase in progress, a detached HEAD); null otherwise */
  syncDeferred: SyncDeferral | null
  /** What the running sync of the session does; null while none runs */
  syncStep: SyncStep | null
  /** The repository whose branches the store holds, set by the first request of a sync that ended; null before it */
  syncedRepositoryPath: string | null
  /** The version of the session's view the store holds (`data-view-version`); null without one */
  viewVersion: Version | null
  /**
   * Counts the patches that changed what the table of the backend is made of (the branches, their tips and remote
   * branches, the stacks, the sections), not the state of a run or of a build: what the backend answers about the table
   * (the repository view, the conflict fix) is asked again only when it changed
   */
  tableRevision: number
  /**
   * Counts the patches that changed the commits of the table and how they are grouped and built (the header, the order,
   * the heads, commits and copies of the branches, the stacks but their remote status, the unassigned commits): what
   * the conflict fix is looked for with. A push or the deletion of an archived branch changes `tableRevision`, not this
   */
  commitsRevision: number
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
   * sync rebuilds `branches` and must not reset a running push. Only the spinner, of a push that waits for a sync too:
   * the remote status a push leaves comes in a patch of the session, which orders it against the syncs (Rust's
   * `TableState::end_push`)
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
  /** The run of the session (`requestSync`) whose grouping `branches` holds: their counts tell how far it is; 0 before one */
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
  syncDeferred: null,
  syncStep: null,
  syncedRepositoryPath: null,
  viewVersion: null,
  tableRevision: 0,
  commitsRevision: 0,
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

// ---- pushes: the spinner state of the page, the remote status is the session's ----

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
  | "action"
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
    action: update.action,
  }
}

// ---- sync ----

/**
 * Syncs the selected repository: asks its session (`src/model/store.ts`) for a sync run for `reason` and resolves when
 * the run that serves the request ended and the table has its last patch. The session runs one sync at a time; a request
 * while one runs is served by one quiet follow-up after it (`afterRewrite`, `manual`), or joins it (`open`).
 * `isSyncing` stays true from the request to the end of that run, so a push waiting for it (`whenSynced`) never pushes
 * refs a sync still rewrites.
 */
export async function syncBranches(reason: SyncReason): Promise<void> {
  await requestSessionSync(reason)
}

/**
 * The sync of ⌘R during a sync (a commit elsewhere during a long sync that started before it): the session checks once
 * after the run whether what a sync reads changed since (a commit, a fetch or a push elsewhere, a regroup by `bd`, a
 * description), and syncs only then; a deferred one (a rebase in the IDE) waits silently. The focus of the window goes
 * to the session itself (`setSessionWindowFocus`), which decides by Auto-sync on Focus
 */
export async function syncIfChanged(): Promise<void> {
  await requestSessionSync("focus")
}

/** Resolves once no sync runs and no request of this window waits: at once while none does */
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
 * `syncBranches` after a rewrite, telling `onStart` the run that serves it (`groupedRun` tells when the branches are its
 * own): an update waits for its own sync this way, and shows how far it is (`selectSyncFraction`)
 */
export async function requestSync(onStart: (run: number) => void = () => {}): Promise<void> {
  onStart(nextRunNumber())
  await requestSessionSync("afterRewrite")
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
