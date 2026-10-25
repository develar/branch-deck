/**
 * The page's side of a session (docs/design/rust-domain-authority.md, "Представление: снимок и патчи"): the session of
 * a repository in Rust owns the branch table and its syncs; a window subscribes to it (`subscribe_view`, under the
 * window's label: the main window to the selected repository's, the review window of a branch to its repository's,
 * each with a store of its own), takes the view at the subscription and then each patch on the channel, and keeps
 * `branchSyncStore` in the shape the components read. A patch applies only to the view at its base: one of another
 * epoch (the session was opened again) or after a gap asks for the snapshot (`get_snapshot`); one that came before the
 * subscription answered waits for it. A branch record is built again only from the parts a patch changed, so a row
 * that reads an untouched branch does not render again.
 *
 * What a sync shows beyond the table is the page's presentation, derived here: the status texts of the rows, `isSyncing`
 * (a run of the session, or a request of this window that has not answered yet), the count of the runs that ended, how
 * far the run is. A request (`requestSessionSync`, and a focus of the window, `setSessionWindowFocus`) resolves when
 * the run that serves it ended and its last patch reached the store (`SyncAnswer.version`); a command whose result the
 * session applied (a push, a deletion) waits the same way for the version it answers (`whenSessionViewAt`); an answer
 * of another epoch than the view's is of a session the window left while the session open now still has the view's
 * epoch, and else subscribes the window again: its subscription went with the actor of its session (`whenServedViewAt`).
 * The remote status of a row is the session's: Rust orders a push's outcome against the syncs, so the store shows what
 * it gets. The facts of the remote services and the Patronus view the session derives of them come in the same view
 * and go to whoever listens (`onSessionFacts`: the Patronus store of each window); a fact is asked again with
 * `refreshSessionFacts`.
 */
import { Channel } from "@tauri-apps/api/core"
import type {
  ApiError,
  BranchChange,
  BranchStructure as StructureView,
  BranchView,
  Commit,
  CommitSync,
  FactFamily,
  FactRefreshReason,
  News,
  RepositoryView,
  StackBranchUpdate,
  SyncAnswer,
  SyncReason,
  TableChanges,
  Version,
  ViewPatch,
} from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { CommandResult } from "@/lib/command"
import { invokeCommand } from "@/lib/command"
import { getSubjectLine, parseSubject } from "@/lib/commit"
import { getErrorCode } from "@/lib/errors"
import { sameStringRecord, withoutKeys } from "@/lib/record"
import type { SyncDeferral } from "@/lib/sync-deferred"
import { describeSyncDeferral } from "@/lib/sync-deferred"
import { showToast } from "@/lib/toast-message"
import { addTiming } from "@/lib/update-progress"
import { isMainWindow } from "@/lib/window-label"
import { setDemandFocused, setDemandSubscription } from "@/model/demand"
import { applyViewPatch } from "@/model/patch"
import type { BranchStructure } from "@/model/structure"
import { ROOT_STRUCTURE } from "@/model/structure"
import { resetArchivedBranches, settleArchivedPlaceholders, updateFromArchivedNames, updateFromIntegrationInfos } from "@/stores/archived-branches"
import type { Branch, BranchSyncState, StackBranch, SyncedCommit } from "@/stores/branch-sync"
import { branchSyncStore, toRemoteStatus } from "@/stores/branch-sync"
import { hideAfterSync, setLandedStatus } from "@/stores/landed"
import { getEffectiveBranchPrefix, getSelectedRepositoryPath, refreshBranchPrefix } from "@/stores/repository"
import { updateProject } from "@/stores/settings/repositorySettings"
import { getRepositoryState, updateRepositoryState } from "@/stores/settings/repositoryState"
import { resetUnappliedBranches, setUnappliedBranches } from "@/stores/unapplied-branches"

/** The subscription of the window to the session of one repository */
interface Session {
  readonly repositoryPath: string
  /** The view as the session sent it, at its version; null until the subscription answered */
  view: RepositoryView | null
  /** The patches that came while the view is not there yet (before the answer, while a snapshot is asked) */
  early: ViewPatch[]
  /** The id of the subscription, for `unsubscribe_view`; null until it answered */
  id: number | null
  /** Why the subscription failed (no branch prefix, the repository went away): no patch comes; null while it holds */
  failure: string | null
  closed: boolean
  /** A snapshot is asked (`get_snapshot`): the patches wait for it */
  refreshing: boolean
  /**
   * The sync requests of this window to the session that have not answered yet: Sync is busy from the click, before the
   * run's first patch. A subscription made again for a session that went away (`resubscribe`) shares the count of the
   * one it replaces, whose requests are still in flight
   */
  readonly pending: { count: number }
  /** Resolves once the subscription answered or failed */
  ready: Promise<void>
  /** The id of the last news of the session the window told (`onSessionFacts`); null until its first view */
  toldNews: number | null
}

let session: Session | null = null
/** Told after every change of the view, the close of a session and a failed subscription */
const viewListeners = new Set<() => void>()

/**
 * Takes the view of the session of `repositoryPath` after a change of its facts or of its Patronus view (`changes` null
 * for a snapshot: everything changed) and the news of the session the window has not told yet: none of those its first
 * view had, each once
 */
export type FactsListener = (repositoryPath: string, view: RepositoryView, changes: TableChanges | null, news: readonly News[]) => void

const factsListeners = new Set<FactsListener>()

/**
 * Tells `listener` the facts and the Patronus view of the session's view now (as a snapshot, when the window has one)
 * and after every change of them; returns the unsubscribe
 */
export function onSessionFacts(listener: FactsListener): () => void {
  factsListeners.add(listener)
  if (session?.view != null) {
    listener(session.repositoryPath, session.view, null, [])
  }
  return () => {
    factsListeners.delete(listener)
  }
}
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
  setDemandSubscription(null)
  resetTable()
  notifyViewListeners()
}

/** Resolves once the subscription of the window answered (or failed); at once without one */
export function whenSessionReady(): Promise<void> {
  return session?.ready ?? Promise.resolve()
}

/**
 * Resolves once the store has the view of the session of `repositoryPath` at `version` (the answer of a command whose
 * result the session applied: a push, the deletion of an archived or unapplied branch), so what runs next reads the
 * table with it; at once for no version (no session was open), and once the window holds no session of that repository
 * (`whenServedViewAt`)
 */
export async function whenSessionViewAt(repositoryPath: string, version: Version | null): Promise<void> {
  if (version !== null) {
    await whenServedViewAt(repositoryPath, version, false)
  }
}

/**
 * Asks the session of `repositoryPath` to read a fact again (`refresh_facts`) and resolves once the patch of that read is
 * in the store; false when nothing was asked (the rules held it back, no session is open) or the request failed
 */
export async function refreshSessionFacts(repositoryPath: string, family: FactFamily, reason: FactRefreshReason, subject?: string): Promise<boolean> {
  const result = await commands.refreshFacts({ repositoryPath, family, reason, ...(subject !== undefined && { subject }) })
  if (result.status === "error") {
    console.warn("Cannot ask the session to read its facts again", result.error)
    return false
  }
  await whenSessionViewAt(repositoryPath, result.data)
  return result.data !== null
}

function subscribe(repositoryPath: string, pending: { count: number } = { count: 0 }): void {
  const opened: Session = {
    repositoryPath,
    view: null,
    early: [],
    id: null,
    failure: null,
    closed: false,
    refreshing: false,
    pending,
    ready: Promise.resolve(),
    toldNews: null,
  }
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
      // what the window shows goes to its new subscription
      setDemandSubscription(opened.id)
      install(opened, result.data.view)
    },
    (error: unknown) => fail(opened, error),
  )
}

/**
 * The session the window subscribed to is gone, and its patches with it: the window subscribes to the session of the
 * repository open now (another epoch), the table kept until its view comes, Sync busy for the requests still in flight
 */
function resubscribe(gone: Session): void {
  gone.closed = true
  if (gone.id !== null) {
    void commands.unsubscribeView({ id: gone.id })
  }
  subscribe(gone.repositoryPath, gone.pending)
}

function fail(failed: Session, error: unknown): void {
  const message = typeof error === "object" && error !== null && "message" in error ? String(error.message) : String(error)
  failed.failure = message || "Cannot subscribe to the branches of the repository"
  if (failed === session) {
    // a repository without a prefix is the welcome card's state, not a failure
    if (getErrorCode(error) !== "no_branch_prefix") {
      console.error("Cannot subscribe to the branches of the repository", error)
    }
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

function applyOrRefresh(target: Session, view: RepositoryView, patch: ViewPatch): void {
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
  projectView(target.repositoryPath, view, next, patch.changes, untoldNews(target, patch.changes.news ?? []))
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
      // the session went away (it was idle, its actor died)
      resubscribe(target)
      return
    }
    install(target, result.data)
  })
}

/** The view of a subscription or a snapshot, then the patches that came meanwhile */
function install(target: Session, view: RepositoryView): void {
  if (target !== session) {
    return
  }
  const previous = target.view
  target.view = view
  // the news of the first view were told before the window came, or are of a session it held before
  const news = target.toldNews === null ? [] : view.news
  target.toldNews ??= view.news.at(-1)?.id ?? 0
  projectView(target.repositoryPath, previous, view, null, untoldNews(target, news))
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
/** Of `news`, those after the last the window told, which it tells now */
function untoldNews(target: Session, news: readonly News[]): readonly News[] {
  const told = target.toldNews ?? 0
  const untold = news.filter(item => item.id > told)
  if (untold.length > 0) {
    target.toldNews = untold.at(-1)!.id
  }
  return untold
}

function projectView(repositoryPath: string, previous: RepositoryView | null, next: RepositoryView, changes: TableChanges | null, news: readonly News[]): void {
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
  update.isSyncing = running || (session?.pending.count ?? 0) > 0
  update.syncError = syncErrorOf(next)
  update.syncDeferred = syncDeferredOf(next, state.syncDeferred)
  update.syncStep = next.sync.kind === "running" ? next.sync.step : null

  if (changes === null || changes.header != null) {
    Object.assign(update, headerFields(state, next))
    const config = next.header.issueNavigationConfig
    // the main window records the issue links of the project; a sub-window of the same session would only repeat it
    if (config !== null && isMainWindow) {
      updateProject(repositoryPath, { issueNavigationConfig: config })
    }
  }
  if (changes === null || changes.order != null) {
    update.branchOrder = next.order
  }
  if (changes === null || changes.rows != null) {
    update.rows = next.rows
  }
  if (changes === null || Object.keys(changes.branches ?? {}).length > 0 || (changes.removedBranches?.length ?? 0) > 0) {
    update.branches = branchesOf(state.branches, next, changes)
  }
  if (changes === null || Object.keys(changes.stacks ?? {}).length > 0 || (changes.removedStacks?.length ?? 0) > 0) {
    update.stackBranches = stackBranchesOf(state.stackBranches, next, changes)
  }
  if (changes === null || Object.keys(changes.pushes ?? {}).length > 0 || (changes.removedPushes?.length ?? 0) > 0) {
    update.pushes = next.pushes
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
  if (changes === null || changes.facts != null || changes.patronus != null || news.length > 0) {
    for (const listener of factsListeners) {
      listener(repositoryPath, next, changes, news)
    }
  }
  if (changesTable(previous, changes)) {
    update.tableRevision = state.tableRevision + 1
  }
  if (changesCommits(previous, changes)) {
    update.commitsRevision = state.commitsRevision + 1
  }
  branchSyncStore.setState(update)
  // a placeholder still waiting got no result, also after a failed run
  if (!running && (wasRunning || changes === null)) {
    settleArchivedPlaceholders()
  }
}

/**
 * Whether a patch changed what the table of the backend is made of (`BranchTable`: the branches, their tips and remote
 * branches, the stacks, the sections): not the state of a run, of the build of a branch or of the copy of a commit,
 * which a sync that changes nothing reports for every branch (Created, then Unchanged)
 */
function changesTable(previous: RepositoryView | null, changes: TableChanges | null): boolean {
  if (changes === null) {
    return true
  }
  const { sync: _sync, branches, ...parts } = changes
  if (Object.values(parts).some(part => !isEmptyPart(part))) {
    return true
  }
  return Object.entries(branches ?? {}).some(
    ([name, change]) =>
      change.head != null || change.commits != null || change.remote != null || (change.copies != null && !sameCopies(previous?.branches[name]?.copies, change.copies)),
  )
}

/**
 * Whether a patch changed the commits of the table and how they are grouped and built (`commitsRevision`): the header,
 * the order, the unassigned commits, a branch or a stack that came or went, the head, the commits or the copies of a
 * branch, a stack beyond its remote status. Not a remote status (a push), the archived, unapplied or landed sections
 */
function changesCommits(previous: RepositoryView | null, changes: TableChanges | null): boolean {
  if (changes === null) {
    return true
  }
  const { header, order, unassigned, branches, removedBranches, stacks, removedStacks } = changes
  if (header != null || order != null || unassigned != null || !isEmptyPart(removedBranches) || !isEmptyPart(removedStacks)) {
    return true
  }
  if (Object.entries(stacks ?? {}).some(([root, update]) => !sameStackBeyondRemote(previous?.stacks[root], update))) {
    return true
  }
  return Object.entries(branches ?? {}).some(
    ([name, change]) => change.head != null || change.commits != null || (change.copies != null && !sameCopies(previous?.branches[name]?.copies, change.copies)),
  )
}

/** The same stack branch, whatever its remote status: a push of it changes nothing it is built of */
function sameStackBeyondRemote(shown: StackBranchUpdate | undefined, update: StackBranchUpdate): boolean {
  if (shown === undefined) {
    return false
  }
  const { remote: _shownRemote, ...built } = shown
  const { remote: _remote, ...next } = update
  return JSON.stringify(built) === JSON.stringify(next)
}

function isEmptyPart(part: unknown): boolean {
  return part == null || (Array.isArray(part) ? part.length === 0 : typeof part === "object" && Object.keys(part).length === 0)
}

/** The same copies: the same hashes, whatever the sync says of each */
function sameCopies(shown: readonly CommitSync[] | undefined, copies: readonly CommitSync[]): boolean {
  return shown !== undefined && shown.length === copies.length && shown.every((copy, index) => copy.hash === copies[index]!.hash)
}

function syncErrorOf(view: RepositoryView | null): string | null {
  if (view?.sync.kind === "failed") {
    return view.sync.error.message || "Failed to sync branches"
  }
  return view?.sync.kind === "running" ? null : requestError
}

/** Why git refused the last sync of `view`, the same object as `shown` while it says the same; null otherwise */
function syncDeferredOf(view: RepositoryView, shown: SyncDeferral | null): SyncDeferral | null {
  if (view.sync.kind !== "deferred") {
    return null
  }
  const deferral = describeSyncDeferral(view.sync.error)
  return shown?.title === deferral.title && shown.description === deferral.description ? shown : deferral
}

/** The fields of the header; a record of the same entries stays the same object, a sync rarely changes them */
function headerFields(state: BranchSyncState, view: RepositoryView): Partial<BranchSyncState> {
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
function projectArchived(previous: RepositoryView | null, next: RepositoryView, changes: TableChanges | null): void {
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
function branchesOf(shown: Readonly<Record<string, Branch>>, view: RepositoryView, changes: TableChanges | null): Record<string, Branch> {
  if (changes === null) {
    return Object.fromEntries(Object.entries(view.branches).map(([name, branch]) => [name, branchOf(shown[name], branch, null)]))
  }
  const branches = withoutKeys(shown, changes.removedBranches ?? [])
  for (const [name, change] of Object.entries(changes.branches ?? {})) {
    const branch = view.branches[name]
    if (branch !== undefined) {
      branches[name] = branchOf(shown[name], branch, change)
    }
  }
  return branches
}

/**
 * The record of a branch of the view. Its commits and their index stay the same arrays while neither the commits nor
 * their copies changed, and its remote status stays the same object while the patch has none for it
 */
function branchOf(shown: Branch | undefined, view: BranchView, change: BranchChange | null): Branch {
  const { head, sync } = view
  const keepCommits = shown !== undefined && change !== null && change.commits == null && change.copies == null
  const keepRemote = shown !== undefined && change !== null && change.remote == null
  const keepStructure = shown !== undefined && change !== null && change.structure == null
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
    structure: keepStructure ? shown.structure : toStructure(view.structure),
  }
}

/** The structure Rust placed the branch with, every field present; a field absent on the wire is empty */
function toStructure(view: StructureView): BranchStructure {
  return {
    parent: view.parent ?? null,
    children: view.children ?? ROOT_STRUCTURE.children,
    ancestors: view.ancestors ?? ROOT_STRUCTURE.ancestors,
    descendants: view.descendants ?? ROOT_STRUCTURE.descendants,
    mainLine: view.mainLine ?? ROOT_STRUCTURE.mainLine,
    parked: view.parked ?? null,
    stackParked: view.stackParked ?? null,
    wholeStack: view.wholeStack ?? null,
    section: view.section ?? "main",
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
function stackBranchesOf(shown: Readonly<Record<string, StackBranch>>, view: RepositoryView, changes: TableChanges | null): Readonly<Record<string, StackBranch>> {
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
    syncDeferred: null,
    syncStep: null,
    syncedRepositoryPath: null,
    inputsCheckedAt: null,
    viewVersion: null,
    baselineBranch: null,
    branchOrder: [],
    rows: { main: [], parked: [] },
    branches: {},
    unassignedCommits: [],
    autosquashPlan: null,
    baselineStatus: null,
    stacksEnabled: false,
    issueGroups: {},
    stackParents: {},
    parked: {},
    stackBranches: {},
    pushes: {},
  })
  resetArchivedBranches()
  resetUnappliedBranches()
}

/** Whether the view of `target` is at `version` or past it, or `target` is no longer the window's session */
function viewReached(target: Session, version: Version): boolean {
  return target !== session || target.failure !== null || (target.view !== null && (target.view.version.epoch !== version.epoch || target.view.version.seq >= version.seq))
}

/** Resolves once `viewReached` holds */
function whenViewAt(target: Session, version: Version): Promise<void> {
  if (viewReached(target, version)) {
    return Promise.resolve()
  }
  return new Promise(resolve => {
    const listener = () => {
      if (viewReached(target, version)) {
        viewListeners.delete(listener)
        resolve()
      }
    }
    viewListeners.add(listener)
  })
}

/**
 * Resolves once the window's session of `repositoryPath` has the view at `version`, the answer of a request (a sync, a
 * focus, a push), with that session; null once the window holds no session of the repository, or one that did not
 * serve the request. An answer of another epoch than the view's is of another session than the window's: of one the
 * window left (a request sent before it opened the session of a new prefix, `useSyncOnOpen`), whose answer it drops,
 * its subscription kept; or of one the request opened again, because the actor of the window's session died and took
 * the subscription with it. Which one, the session open now says (`subscriptionHolds`): when it no longer has the
 * view's epoch, the window subscribes again, once, the table kept until its view comes; a view of yet another epoch
 * then is of a session that did not serve it. While `busy` (a request of Sync), Sync is busy until the view has the
 * version
 */
async function whenServedViewAt(repositoryPath: string, version: Version, busy: boolean): Promise<Session | null> {
  let subscribedAgain = false
  for (;;) {
    const target = session
    if (target === null || target.repositoryPath !== repositoryPath) {
      return null
    }
    await target.ready
    if (target !== session) {
      continue
    }
    if (target.failure !== null || target.view === null) {
      return null
    }
    const viewEpoch = target.view.version.epoch
    if (viewEpoch !== version.epoch) {
      if (subscribedAgain) {
        return null
      }
      const holds = await subscriptionHolds(repositoryPath, viewEpoch)
      // the window opened another session meanwhile, or a snapshot gave its view another epoch: look again
      if (target !== session || target.view.version.epoch !== viewEpoch) {
        continue
      }
      if (holds) {
        return null
      }
      subscribedAgain = true
      resubscribe(target)
      continue
    }
    if (target.view.version.seq >= version.seq) {
      return target
    }
    if (busy) {
      countRequest(target, 1)
      await whenViewAt(target, version).finally(() => countRequest(target, -1))
    } else {
      await whenViewAt(target, version)
    }
  }
}

/**
 * Whether the window's subscription to the session of `repositoryPath` holds: the session open now has the epoch of
 * the view (`get_snapshot`). Not when none is open, or one of another epoch: the window's went away with its actor. A
 * failed read changes nothing: the subscription is kept
 */
async function subscriptionHolds(repositoryPath: string, viewEpoch: number): Promise<boolean> {
  const result = await commands.getSnapshot({ repositoryPath })
  if (result.status === "error") {
    console.error("Cannot read the branches of the repository", result.error)
    return true
  }
  return result.data !== null && result.data.version.epoch === viewEpoch
}

/** Counts a request of `target` that started (1) or answered (-1); Sync of the window shows the requests of its session */
function countRequest(target: Session, delta: 1 | -1): void {
  target.pending.count += delta
  const shown = session
  // the subscription made again for a session that went away counts the requests of the one it replaced
  if (shown !== null && shown.pending === target.pending) {
    branchSyncStore.setState({ isSyncing: shown.view?.sync.kind === "running" || shown.pending.count > 0, syncError: syncErrorOf(shown.view) })
  }
}

/**
 * Asks the session of the window for a sync (`sync`) and resolves when the run that serves the request ended and the
 * store has its last patch, with the answer; null when the request failed. Sync is busy from the request. The last
 * successful sync is recorded for the repository (its time, the branch count, how long it took)
 */
export function requestSessionSync(reason: SyncReason): Promise<SyncAnswer | null> {
  return askSync(reason, reason === "manual")
}

/** `requestSessionSync`; `askAgainOnPrefixChange`: a refusal for the prefix asks once more (`settle`) */
async function askSync(reason: SyncReason, askAgainOnPrefixChange: boolean): Promise<SyncAnswer | null> {
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
    const askAgain = askAgainOnPrefixChange ? () => askSync(reason, false) : null
    return await settle(target, reason, startedAt, () => commands.sync({ repositoryPath: target.repositoryPath, reason }), askAgain)
  } finally {
    countRequest(target, -1)
  }
}

/**
 * Tells the session of the window that the window gained (`true`) or lost the focus (`set_window_focus`, and its demand,
 * which the facts of the session are asked by: `src/model/demand.ts`). Rust decides
 * whether a gained one syncs (Auto-sync on Focus, which it reads itself, and only when what a sync reads changed), and
 * answers when that check, or the run it started, ended: the answer then is recorded as a sync's (`unchanged` says
 * "Checked just now"). Null when nothing was asked (the focus lost, the setting off, no session) or the request failed.
 * Sync is not busy for the check: a run it starts shows in the view
 */
export async function setSessionWindowFocus(focused: boolean): Promise<SyncAnswer | null> {
  setDemandFocused(focused)
  const target = session
  if (target === null) {
    return null
  }
  const startedAt = Date.now()
  await target.ready
  // a window without its session (no prefix, the repository went away) names none to the backend
  if (target !== session || target.failure !== null) {
    return null
  }
  if (!focused) {
    // a lost focus asks for no sync: its failure is no failure of Sync
    const result = await commands.setWindowFocus({ repositoryPath: target.repositoryPath, focused })
    if (result.status === "error") {
      console.error("Cannot tell the session that the window lost the focus", result.error)
    }
    return null
  }
  return settle(target, "focus", startedAt, () => commands.setWindowFocus({ repositoryPath: target.repositoryPath, focused }), null)
}

/**
 * The answer of a request of `target` (`sync`, `set_window_focus`): resolves when the store has the last patch of the
 * run that served it, which may come after the answer (Sync stays busy until then), and records it once the window
 * shows the session that served it (`whenServedViewAt`); null when nothing was asked or the request failed
 */
async function settle(
  target: Session,
  reason: SyncReason,
  startedAt: number,
  request: () => Promise<CommandResult<SyncAnswer | null>>,
  askAgain: (() => Promise<SyncAnswer | null>) | null,
): Promise<SyncAnswer | null> {
  let result: CommandResult<SyncAnswer | null>
  try {
    result = await request()
  } catch (error) {
    failRequest(target, error instanceof Error ? error.message : String(error), error)
    return null
  }
  if (result.status === "error") {
    return refused(target, reason, result.error, askAgain)
  }
  const answer = result.data
  if (answer === null) {
    return null
  }
  const served = await whenServedViewAt(target.repositoryPath, answer.version, true)
  if (served !== null) {
    recordSync(served.repositoryPath, answer, reason, startedAt)
  }
  return answer
}

/**
 * A request of `target` the session refused. A deferred sync (a rebase in progress, a detached HEAD) is no failure: the
 * user's own (`manual`) says so in an `info` toast, any other waits; the view says why it waits (`deferred`), which the
 * page shows where it has no table. Nor is a sync of a session whose prefix the git config no longer names
 * (`prefix_changed`: a `git config` in a terminal): the run resolved the prefix again, the page shows the new one, and
 * the window opens the session under it, which it syncs for a repository synced before (`useSyncOnOpen`). Nothing syncs a repository never synced then, so the user's own sync of it (`askAgain`) is asked
 * again once the page has the prefix: the session of the new prefix serves it
 */
async function refused(target: Session, reason: SyncReason, error: ApiError, askAgain: (() => Promise<SyncAnswer | null>) | null): Promise<SyncAnswer | null> {
  const { code, message } = error
  if (code === "prefix_changed") {
    // only the main window shows the prefix and opens the session under it
    if (!isMainWindow) {
      return null
    }
    const resolved = refreshBranchPrefix()
    if (askAgain === null) {
      return null
    }
    await resolved
    const repositoryPath = target.repositoryPath
    const asksAgain = getSelectedRepositoryPath() === repositoryPath && getEffectiveBranchPrefix() !== "" && getRepositoryState(repositoryPath)?.lastSyncTime === undefined
    return asksAgain ? askAgain() : null
  }
  if (code === "operation_in_progress" || code === "detached_head") {
    if (reason === "manual") {
      showToast({ type: "info", ...describeSyncDeferral(error) })
    }
  } else {
    failRequest(target, message, error)
  }
  return null
}

/** A request of the window's session that failed: Sync names why, until the next run */
function failRequest(target: Session, message: string, error: unknown): void {
  if (target !== session) {
    return
  }
  const shown = message || "Failed to sync branches"
  // a focus that finds what the failed run read answers its error again, without a run: logged once
  if (requestError !== shown) {
    console.error("Sync error:", error)
  }
  requestError = shown
  branchSyncStore.setState({ syncError: syncErrorOf(target.view) })
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
