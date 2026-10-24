/**
 * The page's side of a session (docs/design/rust-domain-authority.md, "Представление: снимок и патчи"): the session of
 * the selected repository in Rust owns the branch table and its syncs; the main window subscribes to it
 * (`subscribe_view`), takes the view at the subscription and then each patch on the channel, and keeps
 * `branchSyncStore` in the shape the components read. A patch applies only to the view at its base: one of another
 * epoch (the session was opened again) or after a gap asks for the snapshot (`get_snapshot`); one that came before the
 * subscription answered waits for it. A branch record is built again only from the parts a patch changed, so a row
 * that reads an untouched branch does not render again.
 *
 * What a sync shows beyond the table is the page's presentation, derived here: the status texts of the rows, `isSyncing`
 * (a run of the session, or a request of this window that has not answered yet), the count of the runs that ended, how
 * far the run is. A request (`requestSessionSync`) resolves when the run that serves it ended and its last patch
 * reached the store (`SyncAnswer.version`).
 */
import { Channel } from "@tauri-apps/api/core"
import type { BranchChange, BranchView, Commit, CommitSync, StackBranchUpdate, SyncAnswer, SyncReason, TableChanges, TableView, Version, ViewPatch } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { getSubjectLine, parseSubject } from "@/lib/commit"
import { sameStringRecord, withoutKeys } from "@/lib/record"
import { showToast } from "@/lib/toast-message"
import { addTiming } from "@/lib/update-progress"
import { applyViewPatch } from "@/model/patch"
import { resetArchivedBranches, settleArchivedPlaceholders, updateFromArchivedNames, updateFromIntegrationInfos } from "@/stores/archived-branches"
import type { Branch, BranchSyncState, StackBranch, SyncedCommit } from "@/stores/branch-sync"
import { branchSyncStore, isBranchSending, toRemoteStatus } from "@/stores/branch-sync"
import { hideAfterSync, setLandedStatus } from "@/stores/landed"
import { getSelectedRepositoryPath } from "@/stores/repository"
import { updateProject } from "@/stores/settings/repositorySettings"
import { updateRepositoryState } from "@/stores/settings/repositoryState"
import { resetUnappliedBranches, setUnappliedBranches } from "@/stores/unapplied-branches"

/** The subscription of the window to the session of one repository */
interface Session {
  readonly repositoryPath: string
  /** The view as the session sent it, at its version; null until the subscription answered */
  view: TableView | null
  /** The patches that came while the view is not there yet (before the answer, while a snapshot is asked) */
  early: ViewPatch[]
  /** The id of the subscription, for `unsubscribe_view`; null until it answered */
  id: number | null
  /** Why the subscription failed (no branch prefix, the repository went away): no patch comes; null while it holds */
  failure: string | null
  closed: boolean
  /** A snapshot is asked (`get_snapshot`): the patches wait for it */
  refreshing: boolean
  /** The sync requests of this window to the session that have not answered yet: Sync is busy from the click, before the run's first patch */
  pending: number
  /** Resolves once the subscription answered or failed */
  ready: Promise<void>
}

let session: Session | null = null
/** Told after every change of the view, the close of a session and a failed subscription */
const viewListeners = new Set<() => void>()
/** The failure of the last request that no run answered (a refused request, no repository); the view has a run's own */
let requestError: string | null = null
/** The runs this window saw start, the number of the one in flight; `groupedRun` names one of them */
let runCount = 0

function notifyViewListeners(): void {
  // a listener that deletes itself is no trouble for the iteration of a Set
  for (const listener of viewListeners) {
    listener()
  }
}

/** The repository of the session the window holds; null without one */
export function sessionRepositoryPath(): string | null {
  return session?.repositoryPath ?? null
}

/** The version of the view the store holds, as `data-view-version` shows it: `epoch:seq`; null without a view */
export function viewVersionKey(version: Version | null): string | null {
  return version === null ? null : `${version.epoch}:${version.seq}`
}

/**
 * Subscribes the window to the session of `repositoryPath`, forgetting the table of the one it held: a switch of the
 * repository or of its branch prefix (another session); the late patches of the old one are dropped by their
 * subscription, which ends here
 */
export function openSession(repositoryPath: string): void {
  closeSession()
  subscribe(repositoryPath)
}

/**
 * Subscribes the window to the session of `repositoryPath` unless its subscription to it holds: the window opens the
 * session of the selected repository at once (the prefix is Rust's), and a sync asked before that opens it itself
 */
export function ensureSession(repositoryPath: string): void {
  if (session === null || session.repositoryPath !== repositoryPath || session.failure !== null) {
    openSession(repositoryPath)
  }
}

/** Ends the subscription of the window and forgets its table: no repository is shown */
export function closeSession(): void {
  const closed = session
  session = null
  if (closed !== null) {
    closed.closed = true
    if (closed.id !== null) {
      void commands.unsubscribeView({ id: closed.id })
    }
  }
  resetTable()
  notifyViewListeners()
}

/** Resolves once the subscription of the window answered (or failed); at once without one */
export function whenSessionReady(): Promise<void> {
  return session?.ready ?? Promise.resolve()
}

function subscribe(repositoryPath: string): void {
  const opened: Session = { repositoryPath, view: null, early: [], id: null, failure: null, closed: false, refreshing: false, pending: 0, ready: Promise.resolve() }
  const channel = new Channel<ViewPatch>()
  channel.onmessage = patch => receive(opened, patch)
  session = opened
  opened.ready = commands.subscribeView({ repositoryPath }, channel).then(
    result => {
      if (result.status === "error") {
        fail(opened, result.error)
        return
      }
      if (opened.closed) {
        void commands.unsubscribeView({ id: result.data.id })
        return
      }
      opened.id = result.data.id
      install(opened, result.data.view)
    },
    (error: unknown) => fail(opened, error),
  )
}

function fail(failed: Session, error: unknown): void {
  const message = typeof error === "object" && error !== null && "message" in error ? String(error.message) : String(error)
  failed.failure = message || "Cannot subscribe to the branches of the repository"
  if (failed === session) {
    console.error("Cannot subscribe to the branches of the repository", error)
    notifyViewListeners()
  }
}

function receive(target: Session, patch: ViewPatch): void {
  if (target !== session) {
    return
  }
  if (target.view === null || target.refreshing) {
    target.early.push(patch)
    return
  }
  applyOrRefresh(target, target.view, patch)
}

function applyOrRefresh(target: Session, view: TableView, patch: ViewPatch): void {
  const at = view.version
  // a patch the snapshot already has
  if (patch.next.epoch === at.epoch && patch.next.seq <= at.seq) {
    return
  }
  if (patch.base.epoch !== at.epoch || patch.base.seq !== at.seq) {
    refresh(target)
    return
  }
  const next = applyViewPatch(view, patch)
  target.view = next
  projectView(target.repositoryPath, view, next, patch.changes)
  notifyViewListeners()
}

/** A patch of another epoch, or after a gap: the view as the session has it now */
function refresh(target: Session): void {
  if (target.refreshing) {
    return
  }
  target.refreshing = true
  void commands.getSnapshot({ repositoryPath: target.repositoryPath }).then(result => {
    target.refreshing = false
    if (target !== session) {
      return
    }
    if (result.status === "error") {
      console.error("Cannot read the branches of the repository", result.error)
      return
    }
    if (result.data === null) {
      // the session went away (it was idle): subscribe to the new one, the table stays until its view comes
      subscribe(target.repositoryPath)
      return
    }
    install(target, result.data)
  })
}

/** The view of a subscription or a snapshot, then the patches that came meanwhile */
function install(target: Session, view: TableView): void {
  if (target !== session) {
    return
  }
  const previous = target.view
  target.view = view
  projectView(target.repositoryPath, previous, view, null)
  const early = target.early
  target.early = []
  for (const patch of early) {
    if (target.view === null || target.refreshing) {
      target.early.push(patch)
    } else {
      applyOrRefresh(target, target.view, patch)
    }
  }
  notifyViewListeners()
}

/**
 * Brings the store to `next`: everything for a snapshot (`changes` null), else what the patch changed. The sections
 * outside this store (the archived, unapplied and landed branches, the issue links of the project) are told too
 */
function projectView(repositoryPath: string, previous: TableView | null, next: TableView, changes: TableChanges | null): void {
  const state = branchSyncStore.getState()
  const update: Partial<BranchSyncState> = { viewVersion: next.version }
  const running = next.sync.kind === "running"
  const wasRunning = previous?.sync.kind === "running"
  if (running && !wasRunning) {
    runCount++
    requestError = null
  }
  if (running && next.sync.kind === "running" && next.sync.step !== "grouping" && state.groupedRun !== runCount) {
    update.groupedRun = runCount
  }
  if (!running && wasRunning) {
    update.syncCount = state.syncCount + 1
  }
  update.isSyncing = running || (session?.pending ?? 0) > 0
  update.syncError = syncErrorOf(next)

  if (changes === null || changes.header != null) {
    Object.assign(update, headerFields(state, next))
    const config = next.header.issueNavigationConfig
    if (config !== null) {
      updateProject(repositoryPath, { issueNavigationConfig: config })
    }
  }
  if (changes === null || changes.order != null) {
    update.branchOrder = next.order
  }
  if (changes === null || Object.keys(changes.branches ?? {}).length > 0 || (changes.removedBranches?.length ?? 0) > 0) {
    update.branches = branchesOf(state.branches, next, changes, repositoryPath)
  }
  if (changes === null || Object.keys(changes.stacks ?? {}).length > 0 || (changes.removedStacks?.length ?? 0) > 0) {
    update.stackBranches = stackBranchesOf(state.stackBranches, next, changes)
  }
  if (changes === null || changes.unassigned != null) {
    update.unassignedCommits = next.sections.unassigned
  }
  if (changes === null || changes.landed != null) {
    const { baseline, landing, landed } = next.sections.landed
    update.baselineStatus = baseline === null ? null : { repositoryPath, ...baseline }
    if (baseline !== null) {
      setLandedStatus(repositoryPath, landing, landed, baseline.update?.upstream ?? baseline.baselineBranch)
    }
  }
  if (changes === null || changes.unapplied != null) {
    setUnappliedBranches(next.sections.unapplied.map(({ remote, ...info }) => ({ ...info, remoteStatus: remote === null ? null : toRemoteStatus(remote) })))
  }
  projectArchived(previous, next, changes)
  if (changes === null || Object.keys(changes).some(part => part !== "sync")) {
    update.tableRevision = state.tableRevision + 1
  }
  branchSyncStore.setState(update)
  // a placeholder still waiting got no result, also after a failed run
  if (!running && (wasRunning || changes === null)) {
    settleArchivedPlaceholders()
  }
}

function syncErrorOf(view: TableView | null): string | null {
  if (view?.sync.kind === "failed") {
    return view.sync.error.message || "Failed to sync branches"
  }
  return view?.sync.kind === "running" ? null : requestError
}

/** The fields of the header; a record of the same entries stays the same object, a sync rarely changes them */
function headerFields(state: BranchSyncState, view: TableView): Partial<BranchSyncState> {
  const { header } = view
  return {
    baselineBranch: header.baselineBranch === "" ? null : header.baselineBranch,
    stacksEnabled: header.stacksEnabled,
    issueGroups: sameStringRecord(header.issueGroups, state.issueGroups) ? state.issueGroups : header.issueGroups,
    stackParents: sameStringRecord(header.stackParents, state.stackParents) ? state.stackParents : header.stackParents,
    parked: sameStringRecord(header.parked, state.parked) ? state.parked : header.parked,
  }
}

/**
 * The archived branches of the view in their store: the names first (placeholders for the new ones), then the
 * integration of the branches the patch changed. "New" marks only what this patch archived: the store keeps the marks
 * the user has not dismissed, and the session's list of every branch archived since it opened would bring them back
 */
function projectArchived(previous: TableView | null, next: TableView, changes: TableChanges | null): void {
  const archived = next.sections.archived
  if (changes === null) {
    resetArchivedBranches()
    updateFromArchivedNames(archived.names, archived.newlyArchived)
    updateFromIntegrationInfos(Object.values(archived.integration))
    return
  }
  const change = changes.archived
  if (change == null) {
    return
  }
  if (change.names != null || change.newlyArchived != null) {
    const marked = new Set(previous?.sections.archived.newlyArchived ?? [])
    updateFromArchivedNames(
      archived.names,
      archived.newlyArchived.filter(name => !marked.has(name)),
    )
  }
  const infos = Object.values(change.integration ?? {})
  if (infos.length > 0) {
    updateFromIntegrationInfos(infos)
  }
}

/** The branch records of `view`: built again only for the branches the patch changed, the others kept as they were */
function branchesOf(shown: Readonly<Record<string, Branch>>, view: TableView, changes: TableChanges | null, repositoryPath: string): Record<string, Branch> {
  if (changes === null) {
    return Object.fromEntries(Object.entries(view.branches).map(([name, branch]) => [name, branchOf(shown[name], branch, null, isBranchSending(name, repositoryPath))]))
  }
  const branches = withoutKeys(shown, changes.removedBranches ?? [])
  for (const [name, change] of Object.entries(changes.branches ?? {})) {
    const branch = view.branches[name]
    if (branch !== undefined) {
      branches[name] = branchOf(shown[name], branch, change, isBranchSending(name, repositoryPath))
    }
  }
  return branches
}

/**
 * The record of a branch of the view. Its commits and their index stay the same arrays while neither the commits nor
 * their copies changed; its remote status stays while the patch has none for it, and while a push of the branch sends:
 * the push's result is the one to show, not what a sync computed mid-push. A push that only waits for the sync sends
 * after it, over the status it brings (a fetch moved the remote branch, and the push goes over the new tip at once)
 */
function branchOf(shown: Branch | undefined, view: BranchView, change: BranchChange | null, sending: boolean): Branch {
  const { head, sync } = view
  const keepCommits = shown !== undefined && change !== null && change.commits == null && change.copies == null
  const keepRemote = shown !== undefined && (sending || (change !== null && change.remote == null))
  return {
    name: head.name,
    commits: keepCommits ? shown.commits : syncedCommits(view.commits, view.copies),
    commitIndex: shown !== undefined && change !== null && change.commits == null ? shown.commitIndex : commitIndexOf(view.commits),
    commitCount: view.commits.length,
    status: sync.status,
    statusText: statusTextOf(view),
    processedCount: sync.processedCount,
    hasError: sync.hasError,
    errorDetails: sync.errorDetails ?? undefined,
    latestCommitTime: head.latestCommitTime,
    summary: head.summary,
    descriptionTitle: head.descriptionTitle ?? null,
    remoteStatus: keepRemote ? shown.remoteStatus : view.remote.status === null ? null : toRemoteStatus(view.remote.status),
    allCommitsHaveIssueReferences: head.allCommitsHaveIssueReferences,
    myEmail: head.myEmail ?? null,
    origin: head.origin,
    issueKey: head.issueKey ?? null,
    stack: head.stack ?? null,
  }
}

/** The commits with what the sync made of each: the hash of its copy, its status, its error, the conflict it resolved */
function syncedCommits(commits: readonly Commit[], copies: readonly CommitSync[]): SyncedCommit[] {
  return commits.map((commit, index) => {
    const copy = copies[index]
    if (copy === undefined || (copy.hash == null && copy.status == null && copy.error == null && copy.resolved == null)) {
      return commit
    }
    const synced: { -readonly [K in keyof SyncedCommit]: SyncedCommit[K] } = { ...commit }
    if (copy.hash != null) {
      synced.hash = copy.hash
    }
    if (copy.status != null) {
      synced.status = copy.status
    }
    if (copy.error != null) {
      synced.error = copy.error
    }
    if (copy.resolved != null) {
      synced.resolved = copy.resolved
    }
    return synced
  })
}

function commitIndexOf(commits: readonly Commit[]): Record<string, number> {
  const index: Record<string, number> = {}
  commits.forEach((commit, position) => {
    index[commit.originalHash] = position
  })
  return index
}

/** The text of a row's status: a stack problem names itself as Rust says it ("conflicting parents"), not as a failure */
function statusTextOf({ head, sync }: BranchView): string {
  switch (sync.status) {
    case "Syncing":
      return "syncing…"
    case "Blocked":
      // not an error of this branch: its parent can't be built on
      return sync.blockedReason ?? "blocked"
    case "Error": {
      const problem = head.stack?.problems[0]
      if (problem !== undefined) {
        return problem.label
      }
      const error = sync.errorDetails
      if (error != null && "MergeConflict" in error) {
        return "merge conflict"
      }
      return error != null && "Generic" in error ? error.Generic : "internal error"
    }
    case "MergeConflict":
      return "merge conflict"
    case "AnalyzingConflict":
      return "analyzing conflict…"
    default:
      return sync.status.toLowerCase()
  }
}

/** The stack branches of the view: built again only for the roots the patch changed */
function stackBranchesOf(shown: Readonly<Record<string, StackBranch>>, view: TableView, changes: TableChanges | null): Readonly<Record<string, StackBranch>> {
  if (changes === null) {
    return Object.fromEntries(Object.values(view.stacks).map(update => [update.root, toStackBranch(update)]))
  }
  const stacks = withoutKeys(shown, changes.removedStacks ?? [])
  for (const update of Object.values(changes.stacks ?? {})) {
    stacks[update.root] = toStackBranch(update)
  }
  return stacks
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

/** Forgets the table of the session the window held: none of its branches shows under the next one's name */
function resetTable(): void {
  autosquashPlanRequest++
  requestError = null
  branchSyncStore.setState({
    isSyncing: false,
    syncError: null,
    syncedRepositoryPath: null,
    inputsCheckedAt: null,
    viewVersion: null,
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

/** Resolves once the view of `target` is at `version` or past it, or `target` is no longer the window's session */
function whenViewAt(target: Session, version: Version): Promise<void> {
  const reached = () =>
    target !== session || target.failure !== null || (target.view !== null && (target.view.version.epoch !== version.epoch || target.view.version.seq >= version.seq))
  if (reached()) {
    return Promise.resolve()
  }
  return new Promise(resolve => {
    const listener = () => {
      if (reached()) {
        viewListeners.delete(listener)
        resolve()
      }
    }
    viewListeners.add(listener)
  })
}

/** Counts a request of `target` that started (1) or answered (-1); Sync of the window shows the requests of its session */
function countRequest(target: Session, delta: 1 | -1): void {
  target.pending += delta
  if (target === session) {
    branchSyncStore.setState({ isSyncing: target.view?.sync.kind === "running" || target.pending > 0, syncError: syncErrorOf(target.view) })
  }
}

/**
 * Asks the session of the window for a sync (`sync`) and resolves when the run that serves the request ended and the
 * store has its last patch, with the answer; null when the request failed. A deferred sync (a rebase in progress, a
 * detached HEAD) is no failure: the user's own (`manual`) says so in an `info` toast, any other waits silently. The
 * last successful sync is recorded for the repository (its time, the branch count, how long it took)
 */
export async function requestSessionSync(reason: SyncReason): Promise<SyncAnswer | null> {
  // the request is for the selected repository: one before the window subscribed (a click while the page still loads
  // the prefix), or after its subscription failed (no prefix then, an IPC failure), subscribes to its session first
  const selectedPath = getSelectedRepositoryPath()
  if (selectedPath === "") {
    requestError = "Please enter a repository path"
    branchSyncStore.setState({ syncError: requestError })
    return null
  }
  ensureSession(selectedPath)
  const target = session!
  const startedAt = Date.now()
  requestError = null
  countRequest(target, 1)
  try {
    await target.ready
    if (target.failure !== null) {
      if (target === session) {
        requestError = target.failure
      }
      return null
    }
    const result = await commands.sync({ repositoryPath: target.repositoryPath, reason })
    if (result.status === "error") {
      const { code, message } = result.error
      if (code === "operation_in_progress" || code === "detached_head") {
        if (reason === "manual") {
          showToast({
            type: "info",
            title: message,
            description: code === "operation_in_progress" ? "The branches sync once git finished it" : "Check out a branch to sync its commits",
          })
        }
      } else if (target === session) {
        requestError = message || "Failed to sync branches"
        console.error("Sync error:", result.error)
      }
      return null
    }
    await whenViewAt(target, result.data.version)
    if (target === session && target.failure === null) {
      recordSync(target.repositoryPath, result.data, reason, startedAt)
    }
    return result.data
  } catch (error) {
    if (target === session) {
      requestError = (error instanceof Error ? error.message : String(error)) || "Failed to sync branches"
      console.error("Sync error:", error)
    }
    return null
  } finally {
    countRequest(target, -1)
  }
}

/** What a request that ended tells beyond the table: the repository is synced, or a focus found nothing changed */
function recordSync(repositoryPath: string, answer: SyncAnswer, reason: SyncReason, startedAt: number): void {
  if (answer.outcome === "skipped") {
    return
  }
  if (answer.outcome === "unchanged") {
    branchSyncStore.setState({ syncedRepositoryPath: repositoryPath, inputsCheckedAt: reason === "focus" ? Date.now() : branchSyncStore.getState().inputsCheckedAt })
    return
  }
  // the usual time of a sync is the share of the sync of an update (`update-progress.ts`): every sync teaches it
  // (`repositoryState`, a file of its own, so a sync never rewrites the user's settings)
  const syncedAt = Date.now()
  updateRepositoryState(repositoryPath, state => ({
    ...state,
    lastSyncTime: syncedAt,
    lastBranchCount: branchSyncStore.getState().branchOrder.length,
    updateTimings: addTiming(state.updateTimings, "sync", syncedAt - startedAt),
  }))
  branchSyncStore.setState({ syncedRepositoryPath: repositoryPath, inputsCheckedAt: null })
  hideAfterSync(repositoryPath)
  void loadAutosquashPlan(repositoryPath)
}

/** The run a request made now is served by: the next one to start (a run in flight serves it with its follow-up) */
export function nextRunNumber(): number {
  return runCount + 1
}

function isFixupCommit(commit: Commit): boolean {
  return parseSubject(getSubjectLine(commit.message)).autosquash !== ""
}

let autosquashPlanRequest = 0

/** Loads the plan only if a commit is a fixup: most syncs find none and need no extra git process */
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
