import { useCallback, useMemo, useSyncExternalStore } from "react"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { createStore } from "zustand/vanilla"
import type {
  ErrorCode,
  ActionState,
  AutosquashPlan,
  BranchError,
  BranchOrigin,
  BranchPush,
  BranchStack,
  BranchStatus,
  Commit,
  CommitSyncStatus,
  DroppedCommit,
  IssueLead,
  LandDecisions,
  Landed,
  LandingRoute,
  NextStepView,
  OperationId,
  PullRequestView,
  PushAction,
  Reason,
  RobotAttention,
  RunningOperation,
  StackBranchAlong,
  StackBranchConflict,
  StackBranchState,
  StackMemberBlock,
  SyncReason,
  SyncStep,
  UpdateCheck,
  Version,
  WholeStackView,
  WordResolutionSummary,
} from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { ParkedState } from "@/lib/parked"
import { stackIslandRoots } from "@/lib/stack-collapse"
import type { StackLanes } from "@/lib/stack-lanes"
import { layoutStack } from "@/lib/stack-lanes"
import type { SyncDeferral } from "@/lib/sync-deferred"
import type { PushProgress } from "@/lib/transfer-progress"
import { lockWaitProgress } from "@/lib/transfer-progress"
import { stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { requestSessionSync } from "@/model/store"
import type { BranchStructure, TableRows } from "@/model/structure"
import { NO_ROWS, ROOT_STRUCTURE } from "@/model/structure"
import { presentOperationLockWait } from "@/presenters/lock-wait"
import { getSelectedProject, useSelectedProjectPath } from "@/stores/repository"

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
  /** Of the commits behind, the ones another author made (`others_behind`): someone's work to take in before a push */
  othersBehind: number
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
  /** The title split at the issue key it starts with, which a row that shows the key leaves out; null for none */
  readonly descriptionLead: IssueLead | null
  /** The issue key the name starts with, which the name shows; null for none */
  readonly nameIssueKey: string | null
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
  /** Where the branch is among the others, as Rust placed it (`BranchView.structure`) */
  readonly structure: BranchStructure
}

/** The structure of `name` as the view placed it; `ROOT_STRUCTURE` for a branch the table lacks */
export function structureIn(state: BranchSyncState, name: string): BranchStructure {
  return state.branches[name]?.structure ?? ROOT_STRUCTURE
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
  /**
   * The code of the error of the last run of the session when it failed (`SyncState::Failed`): `no_identity` is the git
   * alert's (`GitSetupAlert`), not the error alert's; null otherwise
   */
  syncErrorCode: ErrorCode | null
  /** The last sync git refused before it built anything (a rebase in progress, a detached HEAD); null otherwise */
  syncDeferred: SyncDeferral | null
  /** What the running sync of the session does; null while none runs */
  syncStep: SyncStep | null
  /**
   * The repository whose branches the store holds as a sync of its session committed them (the view has a table of this
   * run: a HEAD, not the frame of an earlier run); null before it
   */
  syncedRepositoryPath: string | null
  /** The rows are the frame an earlier run saved (`RepositoryView.cached`), until the first sync of the session commits */
  cached: boolean
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
  /** When a focus last found the inputs as the last sync read them (ms): the header says "Checked just now" then, a minute after the sync */
  inputsCheckedAt: number | null
  /** Repository's baseline branch (e.g., "origin/master") */
  baselineBranch: string | null
  /** Branch names in backend order */
  branchOrder: string[]
  /**
   * The branches of the main table and of the Parked card, in branch order: the order split by the section Rust placed
   * each branch in (`rowsOf`, `BranchStructure.section`)
   */
  rows: TableRows
  branches: Record<string, Branch>
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
  /**
   * Branch -> what its pushes send besides it, as Rust orders them (`RepositoryView.pushes`, `domain_policy::push`): a
   * branch not listed pushes alone; read through `pushOrdersIn`
   */
  pushes: Readonly<Record<string, BranchPush>>
  /**
   * The open pull request of each subject (a branch, `stack:<root>` the whole stack) as Rust reads it
   * (`RepositoryView.openPullRequests`: where it goes, a draft, the code owners Request Review… offers); a subject
   * without an open pull request is not listed
   */
  openPullRequests: Readonly<Record<string, PullRequestView>>
  /**
   * What a Safe Push and a Dry Run of each subject (a branch, `stack:<root>` the whole stack) do now and what stands in
   * their way, as Rust decides their default intents (`RepositoryView.lands`, `domain_policy::land::decide`); none
   * before the Patronus view
   */
  lands: Readonly<Record<string, LandDecisions>>
  /**
   * What landed each subject (a branch, `stack:<root>` the whole stack), as Rust decides it (`RepositoryView.landedStates`,
   * `domain_policy::landed::landed_state`): the fetched baseline, merged Safe Pushes (also a part of it, the commits
   * after it the work left), its pull request merged with its commits; a subject nothing landed is not listed
   */
  landedStates: Readonly<Record<string, Landed>>
  /**
   * The next step and the node of each subject (a branch, `stack:<root>` the whole stack of a root that forks), as Rust
   * decides them (`RepositoryView.nextSteps`, `domain_policy::next_step`, `domain_policy::landing_node`); read through
   * `useRowPart(subject, "landing")`
   */
  nextSteps: Readonly<Record<string, NextStepView>>
  /**
   * The actions of each subject (a branch, `stack:<root>` the whole stack, `robot:<id>` a robot) and whether each can run
   * now, as Rust decides them (`RepositoryView.actions`, `domain_policy::actions`); presented by `src/presenters/actions.ts`
   */
  actions: Readonly<Record<string, readonly ActionState[]>>
  /**
   * The subjects (a branch, `stack:<root>` the whole stack) whose robot needs the user, as Rust decides it
   * (`RepositoryView.attention`, `domain_policy::attention`): the landing overview, Attention and the Dock badge list
   * them (`collectRobotOverview`)
   */
  attention: Readonly<Record<string, RobotAttention>>
  /**
   * The operations of the remote that run or wait in the session, by id (`RepositoryView.operations`, written by
   * `operations::registry`): the moves of rows they make are laid over the nodes (`src/presenters/running-move.ts`)
   */
  operations: Readonly<Record<OperationId, RunningOperation>>
  /**
   * The route of the repository into master with its gate, as Rust decides it (`RepositoryView.route`,
   * `domain_policy::landing_route`); null before a view
   */
  route: LandingRoute | null
}

/**
 * The parts of the view by subject before a view has any (`pushes`, `openPullRequests`, `lands`, `landedStates`,
 * `nextSteps`, `actions`, `attention`), its operations and its route
 */
export const NO_SUBJECT_PARTS: Pick<BranchSyncState, "pushes" | "openPullRequests" | "lands" | "landedStates" | "nextSteps" | "actions" | "attention" | "operations" | "route"> = {
  pushes: {},
  openPullRequests: {},
  lands: {},
  landedStates: {},
  nextSteps: {},
  actions: {},
  attention: {},
  operations: {},
  route: null,
}

export const branchSyncStore = createStore<BranchSyncState>(() => ({
  isSyncing: false,
  syncCount: 0,
  syncError: null,
  syncErrorCode: null,
  syncDeferred: null,
  syncStep: null,
  syncedRepositoryPath: null,
  cached: false,
  viewVersion: null,
  tableRevision: 0,
  commitsRevision: 0,
  inputsCheckedAt: null,
  baselineBranch: null,
  branchOrder: [],
  rows: NO_ROWS,
  branches: {},
  unassignedCommits: [],
  autosquashPlan: null,
  baselineStatus: null,
  groupedRun: 0,
  stacksEnabled: false,
  issueGroups: {},
  stackParents: {},
  parked: {},
  stackBranches: {},
  ...NO_SUBJECT_PARTS,
}))

/**
 * How far the push of each operation this window runs is (`setOperationPushProgress`), absent before git reports a
 * stage: apart from `branchSyncStore`, because a progress tick would re-run every selector of it
 * (docs/design/rendering-performance.md)
 */
const pushProgressStore = createStore<Readonly<Record<string, PushProgress>>>(() => ({}))

// ---- hooks ----

export function useBranchSync<T>(selector: (state: BranchSyncState) => T): T {
  return useStore(branchSyncStore, selector)
}

// ---- narrow subscriptions: a change wakes only the hooks of what it changed ----

/** The fields of the state that are records by a key (a branch, a subject), whose entries keep their objects while a patch leaves them */
type EntryField = "branches" | "nextSteps" | "actions" | "openPullRequests" | "stackBranches" | "pushes" | "landedStates"
type EntryOf<F extends EntryField> = BranchSyncState[F][string]
type Listener = () => void

const fieldListeners = new Map<keyof BranchSyncState, Set<Listener>>()
const entryListeners = new Map<EntryField, Map<string, Set<Listener>>>()

/**
 * The one listener of the store for the narrow hooks (`useBranchSyncField`, `useBranchSyncEntry`): the hooks of a field
 * wake only when the field is another object, those of an entry only when the entry is. A patch that changes the sync
 * of three rows wakes the hooks of those rows, not a selector of every row: each hook of `useBranchSync` runs on every
 * change of the store, 7,324 of them on each patch of the first sync of `perf:table` before (B4 of
 * docs/design/rust-domain-authority.md)
 */
branchSyncStore.subscribe((state, previous) => {
  for (const [field, listeners] of fieldListeners) {
    if (state[field] !== previous[field]) {
      notifyAll(listeners)
    }
  }
  for (const [field, entries] of entryListeners) {
    const now: Readonly<Record<string, unknown>> = state[field]
    const before: Readonly<Record<string, unknown>> = previous[field]
    if (now === before) {
      continue
    }
    for (const [key, listeners] of entries) {
      if (now[key] !== before[key]) {
        notifyAll(listeners)
      }
    }
  }
})

function notifyAll(listeners: ReadonlySet<Listener>): void {
  for (const listener of listeners) {
    listener()
  }
}

/** Adds `listener` to the set of `key`; the returned function takes it out, and the set once it is empty */
function addListener<K>(sets: Map<K, Set<Listener>>, key: K, listener: Listener): () => void {
  let listeners = sets.get(key)
  if (listeners === undefined) {
    listeners = new Set()
    sets.set(key, listeners)
  }
  const added = listeners
  added.add(listener)
  return () => {
    added.delete(listener)
    if (added.size === 0 && sets.get(key) === added) {
      sets.delete(key)
    }
  }
}

/** One subscribe function per field, so a hook keeps its subscription across renders */
const fieldSubscribes = new Map<keyof BranchSyncState, (listener: Listener) => () => void>()

/** The subscribe function of `field` (`useBranchSyncField`): its listener runs when the field is another object */
export function subscribeToField(field: keyof BranchSyncState): (listener: Listener) => () => void {
  let subscribe = fieldSubscribes.get(field)
  if (subscribe === undefined) {
    subscribe = listener => addListener(fieldListeners, field, listener)
    fieldSubscribes.set(field, subscribe)
  }
  return subscribe
}

/**
 * `useBranchSync` of one field: `select` gets the field alone, so it reads nothing else, and runs again only when the
 * field is another object (the operations of the session, the baseline). Like any selector, it answers the same value
 * while the field is the same
 */
export function useBranchSyncField<F extends keyof BranchSyncState, T>(field: F, select: (value: BranchSyncState[F]) => T): T {
  const read = () => select(branchSyncStore.getState()[field])
  return useSyncExternalStore(subscribeToField(field), read, read)
}

/** Runs `listener` when the entry `key` of `field` is another object (`useBranchSyncEntry`); the result unsubscribes */
export function subscribeToEntry(field: EntryField, key: string, listener: Listener): () => void {
  let entries = entryListeners.get(field)
  if (entries === undefined) {
    entries = new Map()
    entryListeners.set(field, entries)
  }
  return addListener(entries, key, listener)
}

/**
 * What `select` reads of the entry `key` of a keyed field (a branch, the next step of a subject), undefined without one:
 * runs again only when that entry is another object, which a patch makes only of what it changes (`projectView`)
 */
export function useBranchSyncEntry<F extends EntryField, T>(field: F, key: string, select: (entry: EntryOf<F> | undefined) => T): T {
  const subscribe = useCallback((listener: Listener) => subscribeToEntry(field, key, listener), [field, key])
  const read = () => select(branchSyncStore.getState()[field][key] as EntryOf<F> | undefined)
  return useSyncExternalStore(subscribe, read, read)
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

/** The branch of `branches` that `name` stacks on, as Rust placed it; null for a root or a parent `branches` lacks */
function parentIn(branches: Record<string, Branch>, name: string): string | null {
  const parent = branches[name]?.structure.parent ?? null
  return parent !== null && branches[parent] !== undefined ? parent : null
}

/**
 * What the table rows are laid out from: the order, of each branch its parent, its issue and its section. A sync event
 * (a commit synced, a remote status) changes the `branches` record but not this.
 */
interface TableLayout {
  readonly rows: TableRows
  readonly branches: Record<string, Branch>
  readonly key: string
}

let lastLayout: TableLayout | null = null

/** The layout of `state`, the same object while its rows and branches stay the same */
function layoutOf(state: BranchSyncState): TableLayout {
  const { rows, branches } = state
  const last = lastLayout
  if (last !== null && last.rows === rows && last.branches === branches) {
    return last
  }
  let key = ""
  for (const name of state.branchOrder) {
    const branch = branches[name]
    key += branch === undefined ? `${name}\0?\n` : `${name}\0${branch.structure.parent ?? ""}\0${branch.issueKey ?? ""}\0${branch.structure.section}\n`
  }
  lastLayout = last !== null && last.key === key ? { ...last, rows, branches } : { rows, branches, key }
  return lastLayout
}

/** The branches of `visible` and the branches they stack on, in branch order */
function withStackAncestors(state: BranchSyncState, visible: ReadonlySet<string>): string[] {
  const kept = new Set<string>()
  for (const name of visible) {
    let current: string | null = state.branches[name] === undefined ? null : name
    while (current !== null && !kept.has(current)) {
      kept.add(current)
      current = parentIn(state.branches, current)
    }
  }
  return state.branchOrder.filter(name => kept.has(name))
}

/** The table of a branch: the main one, or the Parked card of the branches kept local */
export type BranchTableSection = "main" | "parked"

/** The branches of `section`: all of them without a parked branch */
function sectionBranches(state: BranchSyncState, section: BranchTableSection): Record<string, Branch> {
  if (state.rows.parked.length === 0) {
    return state.branches
  }
  return Object.fromEntries(Object.entries(state.branches).filter(([, branch]) => branch.structure.section === section))
}

/** The rows of `branchTableRowsIn`, computed: the stack graph is laid out over the rows that remain */
function computeBranchTableRows(state: BranchSyncState, visible: ReadonlySet<string> | null, section: BranchTableSection = "main"): BranchTableRow[] {
  // a branch parked with its parent stays under it in the Parked card; a parked child of a branch to land is a root there
  const branches = sectionBranches(state, section)
  const branchOrder = (visible === null ? state.branchOrder : withStackAncestors(state, visible)).filter(name => branches[name] !== undefined)
  // the branches stacked on each branch, newest first; a branch in a cycle of parents is a root
  const children = new Map<string, string[]>()
  const isRoot = (name: string) => {
    const seen = new Set<string>([name])
    let parent = parentIn(branches, name)
    while (parent !== null) {
      if (seen.has(parent)) {
        return true
      }
      seen.add(parent)
      parent = parentIn(branches, parent)
    }
    return false
  }
  for (const name of branchOrder) {
    const parent = parentIn(branches, name)
    if (parent !== null && !isRoot(name)) {
      const siblings = children.get(parent)
      if (siblings === undefined) {
        children.set(parent, [name])
      } else {
        siblings.push(name)
      }
    }
  }
  const roots = branchOrder.filter(name => parentIn(branches, name) === null || isRoot(name))
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
  readonly layout: TableLayout
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
  const layout = layoutOf(state)
  if (parkedRowsCache !== null && parkedRowsCache.layout.key === layout.key) {
    return parkedRowsCache.rows
  }
  parkedRowsCache = { layout, visible: null, rows: state.rows.parked.length === 0 ? [] : computeBranchTableRows(state, null, "parked") }
  return parkedRowsCache.rows
}

/**
 * The rows of the branches in `visible` (all for null) and of the branches they stack on, which are `dimmed` unless
 * visible themselves; computed again only when the structure or `visible` changes.
 */
function branchTableRowsIn(state: BranchSyncState, visible: ReadonlySet<string> | null): BranchTableRow[] {
  const layout = layoutOf(state)
  const cache = visible === null ? allRowsCache : visibleRowsCache
  if (cache !== null && cache.visible === visible && cache.layout.key === layout.key) {
    return cache.rows
  }
  const next: RowsCache = { layout, visible, rows: computeBranchTableRows(state, visible) }
  if (visible === null) {
    allRowsCache = next
  } else {
    visibleRowsCache = next
  }
  return next.rows
}

/**
 * The main line of the stack on `root`, root first (`BranchStructure.mainLine`): the longest chain, lane 0 of the stack
 * graph. A Safe Push of its last branch lands the whole line in one robot: the Safe Push of a stack island's header
 */
export function mainLineOf(state: BranchSyncState, root: string): string[] {
  return [root, ...structureIn(state, root).mainLine]
}

/** The branches `name` is built on, root first (`BranchStructure.ancestors`, the chain of its effective bases) */
export function getStackAncestors(name: string): readonly string[] {
  return structureIn(branchSyncStore.getState(), name).ancestors
}

/** The whole stack `root` heads (`BranchStructure.wholeStack`): its stack branch while the stack forks, else the last branch of a straight chain */
export function getWholeStack(root: string): WholeStackView | null {
  return structureIn(branchSyncStore.getState(), root).wholeStack
}

/** `getWholeStack`, re-rendering when it changes */
export function useWholeStack(root: string): WholeStackView | null {
  return useBranchSyncEntry("branches", root, branch => (branch?.structure ?? ROOT_STRUCTURE).wholeStack)
}

/**
 * Why the stack branch of `root` can't be pushed or landed now, as Rust decides it (`WholeStackView.forking.notBuilt`;
 * `noStackBranch` while the stack doesn't fork); null while the sync built it: only a built stack branch has the commits
 * of the stack as they are, the stack lands in pieces otherwise
 */
export function getStackNotBuilt(root: string): Reason | null {
  const whole = structureIn(branchSyncStore.getState(), root).wholeStack
  return whole?.kind === "forking" ? (whole.notBuilt ?? null) : { code: "noStackBranch", root }
}

/** Whether `root` heads a stack that forks, whose stack branch lands it */
export function isForkingRoot(state: BranchSyncState, root: string): boolean {
  return structureIn(state, root).wholeStack?.kind === "forking"
}

/**
 * What the pushes of a branch send, root first, the branch last, as Rust orders them (`BranchPush`,
 * `domain_policy::push::branch_push`); the page decides none of it
 */
export interface PushOrders {
  /** Push Stack: the branches it is built on that have work for the remote, up to the nearest one a Safe Push holds */
  readonly stack: readonly string[]
  /** The push of an action (a pull request, a review request): as `stack`, above the nearest branch that asks first */
  readonly action: readonly string[]
  /** What the push button pushes when it pushes the stack (`action`); empty: the branch alone */
  readonly button: readonly string[]
  /** Push with Children; null for a branch nothing but parked branches is stacked on */
  readonly children: ChildrenPushOrder | null
}

/** What Push with Children of a branch sends (`ChildrenPush`) */
export interface ChildrenPushOrder {
  /** The branch while it has work for the remote, then the branches stacked on it that have work, parents first */
  readonly order: readonly string[]
  /** The branches stacked on it whose push drops remote commits: left out, each asks first in its row */
  readonly askFirst: readonly string[]
  /** Of the root of a stack that forks: what the push does with the stack branch of the whole stack */
  readonly stackBranch: StackBranchAlong | null
  /** It pushes more than the branch itself */
  readonly offers: boolean
}

const NO_PUSH: BranchPush = {}

/**
 * The pushes of `name` (`PushOrders`). A list the view leaves out is the one of the structure (every ancestor, every
 * branch stacked on it); one the view has not ordered yet, while the first sync runs, sends only what has a remote
 * status
 */
export function pushOrdersIn(state: BranchSyncState, name: string): PushOrders {
  const push = state.pushes[name] ?? NO_PUSH
  const structure = structureIn(state, name)
  const known = (names: readonly string[]) => names.filter(it => state.branches[it]?.remoteStatus != null)
  const stack = [...(push.stack ?? known(structure.ancestors)), name]
  const action = push.action == null ? stack : [...push.action, name]
  const children = push.children
  return {
    stack,
    action,
    button: push.button === true ? action : [],
    children:
      children == null
        ? null
        : {
            order: children.order ?? [name, ...structure.descendants],
            askFirst: children.askFirst ?? [],
            stackBranch: children.stackBranch ?? null,
            offers: children.offers === true,
          },
  }
}

/** `pushOrdersIn` of the stores now */
export function getPushOrders(name: string): PushOrders {
  return pushOrdersIn(branchSyncStore.getState(), name)
}

/** The open pull request of `subject` (a branch, `stackReadinessKey(root)`) as Rust reads it; undefined without one */
export function getOpenPullRequest(subject: string): PullRequestView | undefined {
  return branchSyncStore.getState().openPullRequests[subject]
}

/** `getOpenPullRequest`, re-rendering when it changes */
export function useOpenPullRequest(subject: string): PullRequestView | undefined {
  return useBranchSyncEntry("openPullRequests", subject, pullRequest => pullRequest)
}

/** The decisions of the default starts of `subject` (a branch, `stackReadinessKey(root)`) as Rust decides them; undefined before the Patronus view */
export function getLands(subject: string): LandDecisions | undefined {
  return branchSyncStore.getState().lands[subject]
}

/** What landed `subject` (a branch, `stackReadinessKey(root)`) as Rust decides it; undefined while nothing did */
export function getLandedState(subject: string): Landed | undefined {
  return branchSyncStore.getState().landedStates[subject]
}

/** `getLandedState`, re-rendering when it changes */
export function useLandedState(subject: string): Landed | undefined {
  return useBranchSyncEntry("landedStates", subject, landed => landed)
}

/**
 * The subjects whose robot needs the user, as Rust decides it (`RepositoryView.attention`); the same object while no
 * patch changes it
 */
export function useRobotAttention(): Readonly<Record<string, RobotAttention>> {
  return useStore(branchSyncStore, state => state.attention)
}

/** The route of the repository into master with its gate, as Rust decides it (`RepositoryView.route`); null before a view */
export function getLandingRoute(): LandingRoute | null {
  return branchSyncStore.getState().route
}

/** `getLandingRoute` of a component: the same object while no patch changes it */
export function useLandingRoute(): LandingRoute | null {
  return useStore(branchSyncStore, state => state.route)
}

/** The branches stacked directly on `name`, newest first */
export function getStackChildren(name: string): readonly string[] {
  return structureIn(branchSyncStore.getState(), name).children
}

/** The branches stacked on `name`, directly or further down */
export function getStackDescendants(name: string): readonly string[] {
  return structureIn(branchSyncStore.getState(), name).descendants
}

/** `getStackDescendants` without the parked ones and those parked with them: what lands with the stack of `name` */
export function getUnparkedStackDescendants(name: string): string[] {
  const state = branchSyncStore.getState()
  return structureIn(state, name).descendants.filter(it => structureIn(state, it).parked === null)
}

/** Whether the last grouping read prefixes as stacked branches */
export function useStacksEnabled(): boolean {
  return useBranchSyncField("stacksEnabled", enabled => enabled)
}

export function isStacksEnabled(): boolean {
  return branchSyncStore.getState().stacksEnabled
}

/** The issue groups of the last grouping: issue key -> branch name */
export function useIssueGroups(): Readonly<Record<string, string>> {
  return useBranchSyncField("issueGroups", groups => groups)
}

export function getIssueGroups(): Readonly<Record<string, string>> {
  return branchSyncStore.getState().issueGroups
}

/** The stack parents of the last grouping: child -> parent */
export function useStackParents(): Readonly<Record<string, string>> {
  return useBranchSyncField("stackParents", parents => parents)
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
  return branchSyncStore.getState().rows.main
}

/** Why the branch stays local: its own line in the parked branches, or a parked branch it is built on; null otherwise */
export function getParkedState(name: string): ParkedState | null {
  return structureIn(branchSyncStore.getState(), name).parked
}

/** `getParkedState` of the row, re-rendered only when it changes */
export function useParkedState(name: string): ParkedState | null {
  return useBranchSyncEntry("branches", name, branch => (branch?.structure ?? ROOT_STRUCTURE).parked)
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
  return useBranchSyncEntry("branches", name, branch => branch)
}

/** The commit the local virtual branch points to; undefined before the sync created it. Not re-rendered by other changes of the branch. */
export function useBranchTip(name: string): string | undefined {
  return useBranchSyncEntry("branches", name, branch => branch?.commits[0]?.hash)
}

/** The remote status of the branch, null until loaded; re-rendered only when a new status replaces it */
export function useBranchRemoteStatus(name: string): RemoteStatus | null {
  return useBranchSyncEntry("branches", name, branch => branch?.remoteStatus ?? null)
}

/**
 * Whether a push of `name` (a branch, `stackReadinessKey(root)` the stack branch of a whole stack, the full name of an
 * unapplied branch) runs in the selected repository: an operation of its session that pushes it (`pushOperationIn`)
 */
export function useIsBranchPushing(name: string): boolean {
  return useBranchSyncField("operations", operations => pushOperationIn(operations, name) !== undefined)
}

/** Whether the ref `ref` of an operation (`Resource.ref`, its full name) is the one a push of `name` writes */
function isRefOf(ref: string, name: string): boolean {
  const root = stackRootOfReadinessKey(name)
  if (root !== null) {
    return ref.endsWith(`/stack/${root}`)
  }
  return name.includes("/unapplied/") ? ref === name : ref.endsWith(`/virtual/${name}`)
}

/**
 * The operation of the session that pushes `name` now, as the view has it (`RepositoryView.operations`): a push of it
 * (`Push`, from its admission on, its waits for another push, a sync and the lock included), or another operation in
 * the push step of its plan (a pull request, a review request, a restart that pushes first); a start of a Safe Push
 * shows its push in its own slot (`useLandRun`)
 */
export function pushOperationIn(operations: Readonly<Record<OperationId, RunningOperation>>, name: string): RunningOperation | undefined {
  return Object.values(operations).find(
    operation =>
      operation.intent.kind !== "land" &&
      (operation.intent.kind === "push" || operation.step.kind === "pushing") &&
      operation.resources.some(resource => resource.kind === "ref" && isRefOf(resource.name, name)),
  )
}

/**
 * The operation of the session that pushes a stack started from the branch `startedFrom` (Push Stack and the push
 * button that pushes the stack, Push with Children, Push Up to Here), as the view has it; undefined while none runs
 */
function stackPushOperationIn(operations: Readonly<Record<OperationId, RunningOperation>>, startedFrom: string): RunningOperation | undefined {
  return Object.values(operations).find(
    operation =>
      operation.intent.kind === "push" &&
      operation.intent.subject.kind === "branch" &&
      operation.intent.subject.name === startedFrom &&
      operation.intent.scope.kind !== "alone" &&
      operation.intent.scope.kind !== "unapplied",
  )
}

/** The virtual branches the refs of an operation name, in their order: what a push of a stack sends, parents first */
function pushedBranchesOf(operation: RunningOperation): readonly string[] {
  return operation.resources.flatMap(resource => {
    const at = resource.kind === "ref" ? resource.name.indexOf("/virtual/") : -1
    return resource.kind === "ref" && at >= 0 ? [resource.name.slice(at + "/virtual/".length)] : []
  })
}

const NO_BRANCHES: readonly string[] = []

/**
 * The branches a push of a stack started from the branch sends, parents first, as its operation reserves them (its
 * branch alone until its plan pushes some); null when none runs. A string key, so the store notifies only when they
 * change
 */
export function useStackPush(startedFrom: string): readonly string[] | null {
  const key = useBranchSyncField("operations", operations => {
    const operation = stackPushOperationIn(operations, startedFrom)
    return operation === undefined ? null : pushedBranchesOf(operation).join("\n")
  })
  return key === null ? null : key === "" ? NO_BRANCHES : key.split("\n")
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
 * How far the running push of the branch in the selected repository is, as the channel of the window that runs it
 * tells it; while its operation waits for another holder of the repository lock, whom for (its step in the view, so
 * every window says it), with Stop Waiting (`cancel_operation`: nothing pushed). Null before git reports a stage and
 * when no push runs
 */
export function useBranchPushProgress(name: string): PushProgress | null {
  const op = useBranchSyncField("operations", operations => pushOperationIn(operations, name)?.id ?? null)
  const waitingFor = useBranchSyncField("operations", operations => presentOperationLockWait(pushOperationIn(operations, name)))
  const progress = useStore(pushProgressStore, state => (op === null ? undefined : state[op]) ?? null)
  if (op === null || waitingFor === null) {
    return progress
  }
  return lockWaitProgress(waitingFor, () => void commands.cancelOperation({ op }))
}

/**
 * Records how far the push of the operation `op` is, as the channel of the window that runs it tells it (`OperationEvent`
 * progress): the buttons of the branches it pushes show it; null forgets it once the operation ended
 */
export function setOperationPushProgress(op: OperationId, progress: PushProgress | null): void {
  const current = pushProgressStore.getState()
  if (progress === null) {
    if (Object.hasOwn(current, op)) {
      const { [op]: _, ...rest } = current
      pushProgressStore.setState(rest, true)
    }
    return
  }
  pushProgressStore.setState({ [op]: progress })
}

export function useUnassignedCommits(): Commit[] {
  return useBranchSyncField("unassignedCommits", commits => commits)
}

export function useAutosquashPlan(): AutosquashPlan | null {
  return useBranchSyncField("autosquashPlan", plan => plan)
}

export function getAutosquashPlan(): AutosquashPlan | null {
  return branchSyncStore.getState().autosquashPlan
}

/** The update state of the selected repository; null before its first sync */
export function useBaselineStatus(): BaselineStatus | null {
  const repositoryPath = useSelectedProjectPath()
  return useBranchSyncField("baselineStatus", status => (status?.repositoryPath === repositoryPath ? status : null))
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
  return useBranchSyncEntry("stackBranches", root, stackBranch => stackBranch)
}

export function getStackBranch(root: string): StackBranch | undefined {
  return branchSyncStore.getState().stackBranches[root]
}

/**
 * The root whose whole stack a Safe Push of `name` can land instead (the popover offers it), as the view places the
 * whole stacks (`BranchStructure.wholeStack`, by declared parent): the root of the stack of `name` while it forks, else
 * `name` itself while it heads a straight chain; null for a branch outside a stack and for one inside a straight chain,
 * whose whole stack its last branch's Safe Push lands
 */
export function wholeStackRootIn(state: BranchSyncState, name: string): string | null {
  const root = stackRootIn(state.branches, name)
  if (isForkingRoot(state, root)) {
    return root
  }
  return root === name && structureIn(state, name).wholeStack?.kind === "chain" ? name : null
}

/** The stack branch of the stack that forks and holds the branch `name` (`StackBranch.members`); undefined otherwise */
export function getStackBranchOf(name: string): StackBranch | undefined {
  return Object.values(branchSyncStore.getState().stackBranches).find(stack => stack.members.includes(name))
}

/** The root of the stack of `name`: itself without a parent branch; cycle-safe */
function stackRootIn(branches: Record<string, Branch>, name: string): string {
  const seen = new Set<string>([name])
  let root = name
  let parent = parentIn(branches, root)
  while (parent !== null && !seen.has(parent)) {
    seen.add(parent)
    root = parent
    parent = parentIn(branches, root)
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

// ---- pushes: each is an operation of the session, its remote status the session's ----

function selectedRepositoryPath(): string {
  return getSelectedProject()?.path ?? ""
}

/** Whether a push of the branch runs in the repository (by default the selected one; its operations are of the shown one) */
export function isBranchPushing(name: string, repositoryPath: string = selectedRepositoryPath()): boolean {
  return repositoryPath === selectedRepositoryPath() && pushOperationIn(branchSyncStore.getState().operations, name) !== undefined
}

/** Whether a push of a stack started from `startedFrom` runs in the selected repository (`useStackPush`) */
export function isStackPushing(startedFrom: string): boolean {
  return stackPushOperationIn(branchSyncStore.getState().operations, startedFrom) !== undefined
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
