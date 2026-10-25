/**
 * The sessions of the backend for store tests, behind `subscribe_view`, `unsubscribe_view`, `get_snapshot`, `sync` and
 * `set_window_focus` of a mocked `@/lib/bindings`: each repository has a view at a version, which `send` changes by a
 * patch on it and delivers to its subscribers, as the session in Rust does (`repository_model::session`); a sync, and
 * the check of a gained focus while `syncsOnFocus`, waits until the test answers it, with the version of the view then
 */
import type {
  ApiError,
  BranchChange,
  BranchHead,
  Commit,
  RemoteStatusUpdate,
  SetDemandParams,
  SyncAnswer,
  SyncOutcome,
  SyncParams,
  TableChanges,
  TableView,
  Version,
  ViewPatch,
  ViewSubscription,
  WindowFocusParams,
} from "@/lib/bindings"
import { applyViewPatch } from "@/model/patch"

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: ApiError }

interface Subscription {
  readonly id: number
  readonly repositoryPath: string
  readonly channel: { onmessage: (patch: ViewPatch) => void }
}

/**
 * A sync request the test answers: with the outcome and the version of the view then (or `version`: the session that
 * served it is not the one the fake holds now), or with an error
 */
export interface HeldSync {
  readonly params: SyncParams
  answer(outcome?: SyncOutcome, version?: Version): void
  fail(error: ApiError): void
}

/** The view of a session that never synced, at `version` */
export function emptyView(version: Version = { epoch: 1, seq: 0 }): TableView {
  return {
    version,
    sync: { kind: "idle" },
    header: { baselineBranch: "", stacksEnabled: false, issueGroups: {}, stackParents: {}, parked: {}, issueNavigationConfig: null },
    order: [],
    branches: {},
    stacks: {},
    sections: { unassigned: [], archived: { names: [], newlyArchived: [], integration: {} }, unapplied: [], landed: { baseline: null, landing: [], landed: [] } },
    facts: { robots: { head: { value: null, state: { kind: "loading" }, seq: 0, observedAt: 0 }, robots: {}, failing: [], needsReview: {}, problemsFailed: {} } },
    news: [],
  }
}

/** The one commit of the branch `name` as a grouping reports it: `<name>1` */
export function fakeCommit(name: string): Commit {
  return { originalHash: `${name}1`, strippedSubject: "subject", message: "subject", author: "a", authorTime: 1, committerTime: 1 }
}

export function fakeHead(name: string): BranchHead {
  return { name, latestCommitTime: 1, summary: "subject", allCommitsHaveIssueReferences: false, origin: "Prefix" }
}

/** The changes of a grouping of `names`, each a new branch of one commit, the baseline `origin/master` */
export function grouping(...names: string[]): TableChanges {
  const branches: Record<string, BranchChange> = Object.fromEntries(names.map(name => [name, { head: fakeHead(name), commits: [fakeCommit(name)] }]))
  return {
    order: names,
    branches,
    header: { baselineBranch: "origin/master", stacksEnabled: false, issueGroups: {}, stackParents: {}, parked: {}, issueNavigationConfig: null },
  }
}

/** The remote status of `branchName` with `unpushed` commits ahead of its remote branch at `<branchName>-remote` */
export function fakeRemoteStatus(branchName: string, unpushed: number, remoteTip = `${branchName}-remote`): RemoteStatusUpdate {
  const unpushedCommits = Array.from({ length: unpushed }, (_, index) => `${branchName}${index}`)
  return {
    branchName,
    remoteExists: true,
    unpushedCommits,
    commitsBehind: 0,
    commitsAmended: 0,
    commitsRestacked: 0,
    commitsLanded: 0,
    landedCommits: [],
    commitsReworded: 0,
    ancestorCommitsAhead: 0,
    myUnpushedCount: unpushed,
    fastForward: true,
    lastPushTime: 0,
    droppedCommits: [],
    remoteTip,
  }
}

export class FakeSessions {
  readonly views = new Map<string, TableView>()
  readonly subscriptions: Subscription[] = []
  readonly syncs: HeldSync[] = []
  /** The repositories `get_snapshot` was asked for, in order */
  readonly snapshotCalls: string[] = []
  /** Every `subscribe_view`, in order */
  readonly subscribeCalls: { repositoryPath: string }[] = []
  /** While true, `subscribe_view` answers once `releaseSubscriptions` runs: patches sent meanwhile come first */
  holdSubscriptions = false
  /** What `get_snapshot` answers instead of the view, once (`null`: the session went away) */
  snapshotAnswer: TableView | null | undefined = undefined
  /** The error each `subscribe_view` answers instead of a subscription while set (a repository without a prefix) */
  subscribeFailure: ApiError | null = null
  /** Every `set_window_focus`, in order */
  readonly focusCalls: WindowFocusParams[] = []
  /** What the windows told of their demand (`set_demand`) */
  readonly demandCalls: SetDemandParams[] = []
  /** Whether a gained focus checks the session (Auto-sync on Focus, which the backend reads): a held `focus` sync then */
  syncsOnFocus = true
  /** The error each `set_window_focus` answers while set (the prefix went away meanwhile) */
  focusFailure: ApiError | null = null
  private heldSubscriptions: (() => void)[] = []
  private nextId = 1

  reset(): void {
    this.views.clear()
    this.subscriptions.length = 0
    this.syncs.length = 0
    this.snapshotCalls.length = 0
    this.subscribeCalls.length = 0
    this.holdSubscriptions = false
    this.heldSubscriptions = []
    this.snapshotAnswer = undefined
    this.subscribeFailure = null
    this.focusCalls.length = 0
    this.demandCalls.length = 0
    this.syncsOnFocus = true
    this.focusFailure = null
  }

  view(repositoryPath: string): TableView {
    let view = this.views.get(repositoryPath)
    if (view === undefined) {
      view = emptyView()
      this.views.set(repositoryPath, view)
    }
    return view
  }

  /** A patch of `changes` on the view of the session of `repositoryPath`, to each of its subscribers */
  send(repositoryPath: string, changes: TableChanges): ViewPatch {
    const base = this.view(repositoryPath).version
    const patch: ViewPatch = { base, next: { epoch: base.epoch, seq: base.seq + 1 }, changes }
    this.views.set(repositoryPath, applyViewPatch(this.view(repositoryPath), patch))
    this.deliver(repositoryPath, patch)
    return patch
  }

  /** Delivers `patch` as it is to the subscribers of `repositoryPath`, the view of the session unchanged */
  deliver(repositoryPath: string, patch: ViewPatch): void {
    for (const subscription of this.subscriptions.filter(it => it.repositoryPath === repositoryPath)) {
      subscription.channel.onmessage(patch)
    }
  }

  /** The session of `repositoryPath` opened again (it went away while idle): a new epoch, `view` its view */
  reopen(repositoryPath: string, view: TableView): void {
    this.views.set(repositoryPath, view)
  }

  /**
   * The actor of the session of `repositoryPath` died: its subscriptions went with it, and the next request opens the
   * session again, in a new epoch, `view` its view
   */
  dropSession(repositoryPath: string, view: TableView): void {
    for (let index = this.subscriptions.length - 1; index >= 0; index--) {
      if (this.subscriptions[index]!.repositoryPath === repositoryPath) {
        this.subscriptions.splice(index, 1)
      }
    }
    this.views.set(repositoryPath, view)
  }

  releaseSubscriptions(): void {
    const held = this.heldSubscriptions
    this.heldSubscriptions = []
    for (const release of held) {
      release()
    }
  }

  readonly commands = {
    subscribeView: (params: { repositoryPath: string }, channel: { onmessage: (patch: ViewPatch) => void }): Promise<Result<ViewSubscription>> => {
      this.subscribeCalls.push(params)
      if (this.subscribeFailure !== null) {
        return Promise.resolve({ status: "error", error: this.subscribeFailure })
      }
      const subscription = { id: this.nextId++, repositoryPath: params.repositoryPath, channel }
      this.subscriptions.push(subscription)
      const answer = (): Result<ViewSubscription> => ({ status: "ok", data: { id: subscription.id, view: this.view(params.repositoryPath) } })
      if (!this.holdSubscriptions) {
        return Promise.resolve(answer())
      }
      // the view at the subscription, the patches after it go to the channel before the answer
      const view = this.view(params.repositoryPath)
      return new Promise(resolve => this.heldSubscriptions.push(() => resolve({ status: "ok", data: { id: subscription.id, view } })))
    },
    unsubscribeView: async (params: { id: number }): Promise<Result<null>> => {
      const index = this.subscriptions.findIndex(it => it.id === params.id)
      if (index !== -1) {
        this.subscriptions.splice(index, 1)
      }
      return { status: "ok", data: null }
    },
    getSnapshot: async (params: { repositoryPath: string }): Promise<Result<TableView | null>> => {
      this.snapshotCalls.push(params.repositoryPath)
      const answer = this.snapshotAnswer
      this.snapshotAnswer = undefined
      return { status: "ok", data: answer === undefined ? this.view(params.repositoryPath) : answer }
    },
    sync: (params: SyncParams): Promise<Result<SyncAnswer>> => this.hold(params),
    setDemand: async (params: SetDemandParams): Promise<void> => {
      this.demandCalls.push(params)
    },
    setWindowFocus: (params: WindowFocusParams): Promise<Result<SyncAnswer | null>> => {
      this.focusCalls.push(params)
      if (this.focusFailure !== null) {
        return Promise.resolve({ status: "error", error: this.focusFailure })
      }
      if (!params.focused || !this.syncsOnFocus) {
        return Promise.resolve({ status: "ok", data: null })
      }
      return this.hold({ repositoryPath: params.repositoryPath, reason: "focus" })
    },
  }

  private hold(params: SyncParams): Promise<Result<SyncAnswer>> {
    return new Promise(resolve => {
      this.syncs.push({
        params,
        answer: (outcome = "synced", version = this.view(params.repositoryPath).version) => resolve({ status: "ok", data: { outcome, version } }),
        fail: error => resolve({ status: "error", error }),
      })
    })
  }
}

/**
 * The fake sessions a mocked `@/lib/bindings` answers from (`sessionCommands`): one backend for the process, also for
 * a second window whose modules a test loads afresh (`vi.resetModules`), as two windows of the app share one
 */
export const fakeSessions: FakeSessions = ((globalThis as { __fakeSessions?: FakeSessions }).__fakeSessions ??= new FakeSessions())

/** The commands of a mocked `@/lib/bindings` the store of the view calls: the fake sessions', no autosquash plan */
export function sessionCommands() {
  return { ...fakeSessions.commands, getAutosquashPlan: async () => ({ status: "ok", data: null }) }
}
