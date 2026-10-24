/**
 * The sessions of the backend for store tests, behind `subscribe_view`, `unsubscribe_view`, `get_snapshot` and `sync`
 * of a mocked `@/lib/bindings`: each repository has a view at a version, which `send` changes by a patch on it and
 * delivers to its subscribers, as the session in Rust does (`repository_model::session`); a sync waits until the test
 * answers it, with the version of the view then
 */
import type {
  ApiError,
  BranchChange,
  BranchHead,
  Commit,
  RemoteStatusUpdate,
  SyncAnswer,
  SyncOutcome,
  SyncParams,
  TableChanges,
  TableView,
  Version,
  ViewPatch,
  ViewSubscription,
} from "@/lib/bindings"
import { applyViewPatch } from "@/model/patch"

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: ApiError }

interface Subscription {
  readonly id: number
  readonly repositoryPath: string
  readonly channel: { onmessage: (patch: ViewPatch) => void }
}

/** A sync request the test answers: with the outcome and the version of the view then, or with an error */
export interface HeldSync {
  readonly params: SyncParams
  answer(outcome?: SyncOutcome): void
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
    sync: (params: SyncParams): Promise<Result<SyncAnswer>> =>
      new Promise(resolve => {
        this.syncs.push({
          params,
          answer: (outcome = "synced") => resolve({ status: "ok", data: { outcome, version: this.view(params.repositoryPath).version } }),
          fail: error => resolve({ status: "error", error }),
        })
      }),
  }
}

/** The fake sessions a mocked `@/lib/bindings` answers from (`sessionCommands`) */
export const fakeSessions = new FakeSessions()

/** The commands of a mocked `@/lib/bindings` the store of the view calls: the fake sessions', no autosquash plan */
export function sessionCommands() {
  return { ...fakeSessions.commands, getAutosquashPlan: async () => ({ status: "ok", data: null }) }
}
