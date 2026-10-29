/**
 * The page's side of a session (docs/design/rust-domain-authority.md, "Представление: снимок и патчи"): the session of
 * a repository in Rust owns the branch table and its syncs; a window subscribes to it (`subscribe_view`, under the
 * window's label: the main window to the selected repository's, the review window of a branch to its repository's,
 * each with a store of its own), takes the view at the subscription and then each patch on the channel, and keeps
 * `branchSyncStore` in the shape the components read. The main window starts warm (`adoptCachedModel`): the seed the
 * start of the app injected (`window.__BD_MODEL__`, the frame an earlier run saved) is the view before anything is
 * asked, and the window subscribes as the one that holds it, so the answer is the patch from it (`catchUp`), or nothing;
 * a reload of the window installs the same seed and catches up from it again, and the page of the window remembers
 * that it saw a run of that session end (`sessionStorage`, which lives as long as the window), so the first sync ends
 * once in the window. A patch applies only to the view at its base: one of another
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
 * it gets. The facts of the remote services and the Patronus view the session derives of them come in the same view:
 * the fact `auth` is the status of the sign-ins of the window (`applyAuthFacts`), the others go to whoever listens
 * (`onSessionFacts`: the review status of the main window); a fact is asked again with `refreshSessionFacts`.
 */
import { Channel } from "@tauri-apps/api/core"
import type {
  ActionState,
  ApiError,
  BranchChange,
  CachedModel,
  BranchStructure as StructureView,
  BranchView,
  Commit,
  CommitSync,
  FactFamily,
  FactRefreshReason,
  News,
  NextStepView,
  Outcome,
  RepositoryView,
  StackBranchUpdate,
  SyncAnswer,
  SyncReason,
  TableChanges,
  TableWitness,
  Version,
  ViewPatch,
} from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { CommandResult } from "@/lib/command"
import { invokeCommand } from "@/lib/command"
import { isApiError } from "@/lib/errors"
import { sameFields, withoutKeys } from "@/lib/record"
import { toRemoteStatus } from "@/lib/remote-status"
import type { SyncDeferral } from "@/lib/sync-deferred"
import { describeSyncDeferral } from "@/lib/sync-deferred"
import { showToast } from "@/lib/toast-message"
import { addTiming } from "@/lib/update-progress"
import { isMainWindow } from "@/lib/window-label"
import { branchRowFacts } from "@/model/branch-row"
import { setDemandFocused, setDemandSubscription } from "@/model/demand"
import { applyViewPatch } from "@/model/patch"
import type { BranchStructure } from "@/model/structure"
import { NO_ROWS, ROOT_STRUCTURE, rowsOf } from "@/model/structure"
import { resetArchivedBranches, settleArchivedPlaceholders, updateFromArchivedNames, updateFromIntegrationInfos } from "@/stores/archived-branches"
import { applyAuthFacts } from "@/stores/auth"
import type { Branch, BranchSyncState, StackBranch, SyncedCommit } from "@/stores/branch-sync"
import { branchSyncStore, NO_SUBJECT_PARTS, useBranchSyncEntry } from "@/stores/branch-sync"
import { hideAfterSync, setLandedStatus } from "@/stores/landed"
import { getSelectedRepositoryPath, setSessionAnswer } from "@/stores/repository"
import { updateProject } from "@/stores/settings/repositorySettings"
import { getRepositoryState, updateRepositoryState } from "@/stores/settings/repositoryState"
import { resetUnappliedBranches, setUnappliedBranches } from "@/stores/unapplied-branches"

/** The subscription of the window to the session of one repository */
interface Session {
  readonly repositoryPath: string
  /** The view as the session sent it, at its version (the seed of the start until the answer); null until then */
  view: RepositoryView | null
  /** The patches that came before the answer or while a snapshot is asked: they wait for it */
  early: ViewPatch[]
  /** The id of the subscription, for `unsubscribe_view`; null until it answered */
  id: number | null
  /** Why the subscription failed (no branch prefix, no git, the repository went away): no patch comes; null while it holds */
  failure: ApiError | null
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
  /** The version of the seed of the start the subscription began from (`adoptCachedModel`); null for none */
  readonly seed: Version | null
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

/**
 * Calls `withdrawn` once the session of `repositoryPath` no longer offers the Undo of the rewrite that ended with
 * `outcome` (`RepositoryView.undo`: of the last rewrite of this process with an Undo, while HEAD is still the head it
 * left), looking after every change of the view; returns the stop. The view has the offer from the version the outcome
 * answered (`versionAfter`), which the window reached before it showed the outcome; one without it (a request that
 * joined the rewrite) waits until the view showed the offer. A view of another repository says nothing of it: the
 * undo's token moves each ref back with compare-and-swap
 */
export function followUndo(repositoryPath: string, outcome: Pick<Outcome, "op" | "versionAfter">, withdrawn: () => void): () => void {
  let offered = outcome.versionAfter !== null
  const stop = () => {
    viewListeners.delete(check)
  }
  const check = () => {
    const view = session?.repositoryPath === repositoryPath ? session.view : null
    if (view === null) {
      return
    }
    if (view.undo === outcome.op) {
      offered = true
    } else if (offered) {
      stop()
      withdrawn()
    }
  }
  viewListeners.add(check)
  check()
  return stop
}

/** The table of a view before its first grouping: no HEAD, no stores */
const NO_WITNESS: TableWitness = { head: "", storeTips: { issueGroups: null, stackParents: null, parked: null } }

/**
 * The table the window shows of `repositoryPath`, as a rewrite names its commits and branches on it (`TableWitness`):
 * HEAD and the tips of the stores of the last grouping of its view; that of no grouping while the window holds no view
 * of it (the engine then names them on HEAD as it is)
 */
export function getSessionWitness(repositoryPath: string): TableWitness {
  const view = session?.repositoryPath === repositoryPath ? session.view : null
  return view === null ? NO_WITNESS : { head: view.header.head, storeTips: view.header.storeTips }
}

/** The version of the view the store holds, as `data-view-version` shows it: `epoch:seq`; null without a view */
export function viewVersionKey(version: Version | null): string | null {
  return version === null ? null : `${version.epoch}:${version.seq}`
}

/**
 * The start of the main window with `model`, the seed the start of the app injected (`window.__BD_MODEL__`,
 * `src-tauri/src/warm_start.rs`): the view of the frame of the session of the selected project, which Rust opened
 * before the window and whose first sync runs while the page loads. Called before the first render: the rows show on
 * the first frame, and the window subscribes at once, holding the seed, so the session answers the patch from it. A
 * model of another project (a reload after a switch) is left
 */
export function adoptCachedModel(model: CachedModel | undefined): void {
  if (model === undefined || session !== null || model.repositoryPath !== getSelectedRepositoryPath()) {
    return
  }
  subscribe(model.repositoryPath, { count: 0 }, model.view)
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
 * session of the selected repository at once (the prefix is Rust's), and a sync asked before that opens it itself; a
 * failed subscription is asked again
 */
export function ensureSession(repositoryPath: string): void {
  if (session === null || session.repositoryPath !== repositoryPath || session.failure !== null) {
    openSession(repositoryPath)
  }
}

/**
 * Subscribes the window again to the session of its repository, holding the view it shows: the prefix that names its
 * branches may have changed (the app's prefix, a write of Settings → Repository, a `git config` in a terminal that a
 * sync found, `prefix_changed`), and Rust resolves the prefix of a subscription from the config as it is. The same
 * session answers the patch from the view the window holds or nothing, so the rows stay as they are; the session of
 * another prefix answers its view, which replaces them. The subscription it replaces ends once the new one answered,
 * so the session that stays never loses its last window (which would sync it as a window that comes back). A failed
 * subscription is asked again (`openSession`); resolves once the new one answered
 */
export function resubscribeSession(): Promise<void> {
  const held = session
  if (held === null) {
    return Promise.resolve()
  }
  if (held.failure !== null || held.view === null || held.id === null) {
    openSession(held.repositoryPath)
  } else {
    subscribe(held.repositoryPath, held.pending, null, held)
  }
  return session!.ready
}

/** Ends the subscription of the window and forgets its table: no repository is shown */
export function closeSession(): void {
  const closed = session
  session = null
  if (closed !== null) {
    leave(closed)
  }
  setDemandSubscription(null)
  setSessionAnswer(null, null)
  resetTable()
  notifyViewListeners()
}

/** The end of the subscription of `left`: its late patches reach nothing */
function leave(left: Session): void {
  left.closed = true
  if (left.id !== null) {
    void commands.unsubscribeView({ id: left.id })
  }
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

/**
 * Subscribes the window to the session of `repositoryPath`; `held`, the seed of the start (`adoptCachedModel`), is the
 * view the store shows until the answer brings it to the session's. `replaces`, a subscription that holds, is asked
 * again (`resubscribeSession`): its view is the one the store shows already, and it ends once the new one answered
 */
function subscribe(repositoryPath: string, pending: { count: number } = { count: 0 }, held: RepositoryView | null = null, replaces: Session | null = null): void {
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
    toldNews: replaces?.toldNews ?? null,
    seed: held?.version ?? replaces?.seed ?? null,
  }
  const channel = new Channel<ViewPatch>()
  channel.onmessage = patch => receive(opened, patch)
  session = opened
  if (replaces !== null) {
    opened.view = replaces.view
  } else if (held !== null) {
    install(opened, held)
  }
  const have = opened.view?.version
  opened.ready = commands.subscribeView({ repositoryPath, ...(have !== undefined && { have }) }, channel).then(
    result => {
      if (replaces !== null) {
        leave(replaces)
      }
      if (result.status === "error") {
        fail(opened, result.error)
        return
      }
      if (opened.closed) {
        void commands.unsubscribeView({ id: result.data.id })
        return
      }
      const { id, view, catchUp } = result.data
      opened.id = id
      // what the window shows goes to its new subscription
      setDemandSubscription(id)
      if (view !== null) {
        if (replaces !== null && view.version.epoch !== replaces.view?.version.epoch) {
          // the session of another prefix: nothing of the one the window held shows under it, as on a switch
          opened.view = null
          opened.toldNews = null
          resetTable()
        }
        install(opened, view)
      } else if (opened.view !== null) {
        // the seed it holds: the patch from it, or nothing when the session sent none since
        if (catchUp !== null) {
          applyOrRefresh(opened, opened.view, catchUp)
        }
        applyEarly(opened)
      }
    },
    (error: unknown) => {
      if (replaces !== null) {
        leave(replaces)
      }
      fail(opened, error)
    },
  )
}

/**
 * The session the window subscribed to is gone, and its patches with it: the window subscribes to the session of the
 * repository open now (another epoch), the table kept until its view comes, Sync busy for the requests still in flight
 */
function resubscribe(gone: Session): void {
  leave(gone)
  subscribe(gone.repositoryPath, gone.pending)
}

/**
 * The subscription of `failed` was refused or failed: no session serves the window, which shows why by the code
 * (`sessionFailure` of the repository store: the welcome card without a prefix, the git alert without git, the error
 * alert otherwise). A seed of the start it held is a frame of no session the window holds: its rows go
 */
function fail(failed: Session, error: unknown): void {
  failed.failure = isApiError(error) ? error : subscriptionError(error)
  if (failed === session) {
    // a repository without a prefix is the welcome card's state, not a failure
    if (failed.failure.code !== "no_branch_prefix") {
      console.error("Cannot subscribe to the branches of the repository", error)
    }
    if (failed.view !== null) {
      failed.view = null
      resetTable()
    }
    setSessionAnswer("", failed.failure)
    notifyViewListeners()
  }
}

/** A failure of the subscription that is no answer of Rust (the IPC failed), as an `internal` error of its words */
function subscriptionError(error: unknown): ApiError {
  const message = typeof error === "object" && error !== null && "message" in error ? String(error.message) : String(error)
  return { code: "internal", message: message || "Cannot subscribe to the branches of the repository", details: null, retry: "no", operation_id: null }
}

function receive(target: Session, patch: ViewPatch): void {
  if (target !== session) {
    return
  }
  // before the answer: a patch follows what the answer brings the view to, also past a seed the window holds
  if (target.id === null || target.view === null || target.refreshing) {
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
  projectView(target.repositoryPath, view, next, patch.changes, untoldNews(target, patch.changes.news ?? []), target.seed)
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
  projectView(target.repositoryPath, previous, view, null, untoldNews(target, news), target.seed)
  applyEarly(target)
}

/** The patches that came before the view they follow, applied now; those that still wait are kept */
function applyEarly(target: Session): void {
  const early = target.id === null ? [] : target.early
  target.early = target.id === null ? target.early : []
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

/**
 * The key of `sessionStorage` under which the page of a window keeps the seed whose session it saw a run of end
 * (`seedKey`): it lives as long as the window, over a reload, whose init script installs the same seed
 */
const SEED_ENDED_KEY = "bd-seed-ended"

/** The seed `seed` of the session of `repositoryPath`, as the page keeps it: `epoch:seq` and the path */
function seedKey(repositoryPath: string, seed: Version): string {
  return `${viewVersionKey(seed)}\n${repositoryPath}`
}

/** The page of the window, or the one before its reload, saw a run of the session seeded by `seed` end */
function sawRunEnd(repositoryPath: string, seed: Version): boolean {
  try {
    return sessionStorage.getItem(SEED_ENDED_KEY) === seedKey(repositoryPath, seed)
  } catch {
    // no storage (a private context): a reload ends that run once more
    return false
  }
}

function rememberRunEnd(repositoryPath: string, seed: Version): void {
  try {
    sessionStorage.setItem(SEED_ENDED_KEY, seedKey(repositoryPath, seed))
  } catch {
    // no storage: a reload ends that run once more
  }
}

function projectView(
  repositoryPath: string,
  previous: RepositoryView | null,
  next: RepositoryView,
  changes: TableChanges | null,
  news: readonly News[],
  seed: Version | null,
): void {
  // the prefix Rust resolved for the subscription names the branches the page shows
  setSessionAnswer(next.header.prefix, null)
  const state = branchSyncStore.getState()
  const update: Partial<BranchSyncState> = { viewVersion: next.version, cached: next.cached }
  // a table of this run: a sync of the session committed it (the frame of an earlier run and a session no sync built
  // have none)
  if (!next.cached && next.header.head !== "") {
    update.syncedRepositoryPath = repositoryPath
  }
  const running = next.sync.kind === "running"
  const wasRunning = previous?.sync.kind === "running"
  if (running && !wasRunning) {
    runCount++
    requestError = null
  }
  if (running && next.sync.kind === "running" && next.sync.step !== "grouping" && state.groupedRun !== runCount) {
    update.groupedRun = runCount
  }
  // a run ended: the window saw it run, or the first sync of the session committed the table before the window heard
  // of it (the catch-up from the seed, idle on both sides, is the frame of an earlier run no more). A reload of the
  // window catches up from the same seed: that is the end of a run only while no page of the window saw one end (the
  // session can't tell: a channel to a page that went away takes its patches). A snapshot over the seed (a session of
  // another epoch) ends none
  const caughtUp = changes !== null && previous?.cached === true && !next.cached && next.header.head !== ""
  const ended = !running && (wasRunning || (caughtUp && (seed === null || !sawRunEnd(repositoryPath, seed))))
  if (ended && seed !== null) {
    rememberRunEnd(repositoryPath, seed)
  }
  if (ended) {
    update.syncCount = state.syncCount + 1
    // a run no request of this window waits for (the session's own on open, the follow-up of a rewrite, `bd`, another
    // window): the repository is synced now; a request records its own run with its time (`recordSync`)
    if (next.sync.kind === "idle" && isMainWindow && (session?.pending.count ?? 0) === 0) {
      recordSynced(repositoryPath, next.order.length)
    }
  }
  update.isSyncing = running || (session?.pending.count ?? 0) > 0
  update.syncError = syncErrorOf(next)
  update.syncErrorCode = next.sync.kind === "failed" ? next.sync.error.code : null
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
  if (changes === null || Object.keys(changes.branches ?? {}).length > 0 || (changes.removedBranches?.length ?? 0) > 0) {
    update.branches = branchesOf(state.branches, next, changes)
  }
  // the rows of each section: the order split by the section of each branch, which comes with its structure
  if (changes === null || changes.order != null || Object.values(changes.branches ?? {}).some(change => change.structure != null)) {
    update.rows = rowsOf(next.order, update.branches ?? state.branches, state.rows)
  }
  if (changes === null || Object.keys(changes.stacks ?? {}).length > 0 || (changes.removedStacks?.length ?? 0) > 0) {
    update.stackBranches = stackBranchesOf(state.stackBranches, next, changes)
  }
  if (changes === null || Object.keys(changes.pushes ?? {}).length > 0 || (changes.removedPushes?.length ?? 0) > 0) {
    update.pushes = next.pushes
  }
  if (changes === null || Object.keys(changes.openPullRequests ?? {}).length > 0 || (changes.removedOpenPullRequests?.length ?? 0) > 0) {
    update.openPullRequests = next.openPullRequests
  }
  if (changes === null || Object.keys(changes.lands ?? {}).length > 0 || (changes.removedLands?.length ?? 0) > 0) {
    update.lands = next.lands
  }
  if (changes === null || Object.keys(changes.landedStates ?? {}).length > 0 || (changes.removedLandedStates?.length ?? 0) > 0) {
    update.landedStates = next.landedStates
  }
  if (changes === null || Object.keys(changes.nextSteps ?? {}).length > 0 || (changes.removedNextSteps?.length ?? 0) > 0) {
    update.nextSteps = next.nextSteps
  }
  if (changes === null || Object.keys(changes.actions ?? {}).length > 0 || (changes.removedActions?.length ?? 0) > 0) {
    update.actions = next.actions
  }
  if (changes === null || Object.keys(changes.attention ?? {}).length > 0 || (changes.removedAttention?.length ?? 0) > 0) {
    update.attention = next.attention
  }
  if (changes === null || Object.keys(changes.operations ?? {}).length > 0 || (changes.removedOperations?.length ?? 0) > 0) {
    update.operations = next.operations
  }
  if (changes === null || changes.route != null) {
    update.route = next.route
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
  if (changes === null || changes.facts != null) {
    applyAuthFacts(next.facts, changes?.facts ?? null)
  }
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
  if (ended || (!running && changes === null)) {
    // a placeholder still waiting got no result, also after a failed run
    settleArchivedPlaceholders()
    // the fixups of the table the run left, whoever asked it: this window, another, a rewrite of the engine; none of the
    // frame of an earlier run, whose HEAD git may have moved since
    if (!next.cached) {
      void loadAutosquashPlan(repositoryPath)
    }
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
  // the operations that run and the Undo the session offers are no part of the table
  const { sync: _sync, branches, operations: _operations, removedOperations: _removedOperations, undo: _undo, ...parts } = changes
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
    reviewExperimental: header.reviewExperimental,
    issueGroups: sameFields(header.issueGroups, state.issueGroups) ? state.issueGroups : header.issueGroups,
    stackParents: sameFields(header.stackParents, state.stackParents) ? state.stackParents : header.stackParents,
    parked: sameFields(header.parked, state.parked) ? state.parked : header.parked,
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
 * their copies changed, its remote status stays the same object while the patch has none for it, and what its row
 * shows beside its status stays the same object while it is equal (`Branch.row`): a patch of its sync alone renders the
 * status of its row, not the row
 */
function branchOf(shown: Branch | undefined, view: BranchView, change: BranchChange | null): Branch {
  const branch = branchFieldsOf(shown, view, change)
  const row = branchRowFacts(branch)
  return { ...branch, row: shown !== undefined && sameFields(shown.row, row) ? shown.row : row }
}

function branchFieldsOf(shown: Branch | undefined, view: BranchView, change: BranchChange | null): Omit<Branch, "row"> {
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
    descriptionLead: head.descriptionLead ?? null,
    nameIssueKey: head.nameIssueKey ?? null,
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

/**
 * Why the sync did not build the branch as it is (`Branch.statusText`): a stack problem names itself as Rust says it
 * ("conflicting parents"), not as a failure; nothing while it syncs and once it built it, which its status says
 */
function statusTextOf({ head, sync }: BranchView): string {
  switch (sync.status) {
    case "Syncing":
    case "Created":
    case "Updated":
    case "Unchanged":
      return ""
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
    syncErrorCode: null,
    syncDeferred: null,
    syncStep: null,
    syncedRepositoryPath: null,
    cached: false,
    inputsCheckedAt: null,
    viewVersion: null,
    baselineBranch: null,
    branchOrder: [],
    rows: NO_ROWS,
    branches: {},
    unassignedCommits: [],
    autosquashPlan: null,
    baselineStatus: null,
    stacksEnabled: false,
    reviewExperimental: false,
    issueGroups: {},
    stackParents: {},
    parked: {},
    stackBranches: {},
    ...NO_SUBJECT_PARTS,
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
        requestError = target.failure.message
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
  if (target !== session) {
    return null
  }
  // a window without its session (no prefix, no git, the repository went away) names none to the backend; the focus
  // asks the subscription again, as the user may have fixed it meanwhile (a prefix or git installed in a terminal)
  if (target.failure !== null) {
    if (focused) {
      openSession(target.repositoryPath)
    }
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
 * (`prefix_changed`: a `git config` in a terminal): the run read the prefix again, and the window subscribes again
 * (`resubscribeSession`), so the session of the new prefix answers, which syncs as it opens for a repository synced
 * before. Nothing syncs a repository never synced then, so the user's own sync of it (`askAgain`) is asked again once
 * the window holds that session: it serves it
 */
async function refused(target: Session, reason: SyncReason, error: ApiError, askAgain: (() => Promise<SyncAnswer | null>) | null): Promise<SyncAnswer | null> {
  const { code, message } = error
  if (code === "prefix_changed") {
    // only the main window shows the prefix and opens the session under it
    if (!isMainWindow || target !== session) {
      return null
    }
    const resubscribed = resubscribeSession()
    if (askAgain === null) {
      return null
    }
    await resubscribed
    const repositoryPath = target.repositoryPath
    const held = session as Session | null
    const asksAgain = held?.repositoryPath === repositoryPath && held.failure === null && getRepositoryState(repositoryPath)?.lastSyncTime === undefined
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
    branchSyncStore.setState({ inputsCheckedAt: reason === "focus" ? Date.now() : branchSyncStore.getState().inputsCheckedAt })
    return
  }
  // the usual time of a sync is the share of the sync of an update (`update-progress.ts`): every sync teaches it
  recordSynced(repositoryPath, branchSyncStore.getState().branchOrder.length, Date.now() - startedAt)
  branchSyncStore.setState({ inputsCheckedAt: null })
}

/**
 * The repository is synced now, with `branchCount` branches (the room of the loading table at the next start), and
 * the sync took `took` ms when the window knows when it was asked; the Landed island hides what the user saw
 * (`hideAfterSync`). In `repositoryState`, a file of its own, so a sync never rewrites the user's settings
 */
function recordSynced(repositoryPath: string, branchCount: number, took?: number): void {
  const syncedAt = Date.now()
  updateRepositoryState(repositoryPath, state => ({
    ...state,
    lastSyncTime: syncedAt,
    lastBranchCount: branchCount,
    ...(took !== undefined && { updateTimings: addTiming(state.updateTimings, "sync", took) }),
  }))
  hideAfterSync(repositoryPath)
}

/** The run a request made now is served by: the next one to start (a run in flight serves it with its follow-up) */
export function nextRunNumber(): number {
  return runCount + 1
}

function isFixupCommit(commit: Commit): boolean {
  return commit.subjectParts.autosquash !== ""
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

/** The parts of the view a row reads by the key of its subject (a branch, `stack:<root>` the whole stack), by name */
interface RowParts {
  /** The next step and the node of the subject (`RepositoryView.nextSteps`) */
  readonly landing: NextStepView
  /** The actions of the subject and whether each can run now (`RepositoryView.actions`; `robot:<id>` a robot's) */
  readonly actions: readonly ActionState[]
}

/** The record of the store that keeps each part of the rows */
const ROW_PARTS = { landing: "nextSteps", actions: "actions" } as const satisfies { readonly [P in keyof RowParts]: keyof BranchSyncState }

/** The part `part` of the row of `subject` as the view holds it now; undefined while the view has none of it */
export function getRowPart<P extends keyof RowParts>(subject: string, part: P): RowParts[P] | undefined {
  return branchSyncStore.getState()[ROW_PARTS[part]][subject] as RowParts[P] | undefined
}

/**
 * `getRowPart`, re-rendering when it changes: one kept reference, the same object while the patches leave the part of
 * the subject untouched (`applyViewPatch`), so a row renders again only when its own part changes, and its hook runs
 * only then (`useBranchSyncEntry`)
 */
export function useRowPart<P extends keyof RowParts>(subject: string, part: P): RowParts[P] | undefined {
  return useBranchSyncEntry(ROW_PARTS[part], subject, entry => entry as RowParts[P] | undefined)
}
