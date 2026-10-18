import { useEffect, useState } from "react"
import { useStore } from "zustand"
import { getBranchNextStepOf } from "@/hooks/useBranchNextStep"
import { peekDismissesStaleReviews, readDismissesStaleReviews } from "@/hooks/useDismissesStaleReviews"
import type { PatronusProfile } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { pushForAction } from "@/lib/git/push"
import type { BypassProfiles } from "@/lib/patronus"
import { allowsPushMode, formatPushHoldReason, formatPushMode, getBypassProfiles, getPullRequestStart, getStartPush, isStartBlocked } from "@/lib/patronus"
import { openPullRequest } from "@/lib/pull-request"
import { getPushAction } from "@/lib/push-summary"
import type { SafePushDecision, SafePushFacts, ReviewFact, StartOptionsFact } from "@/lib/safe-push-decision"
import { getSafePushDecision } from "@/lib/safe-push-decision"
import type { SafePushStart } from "@/lib/safe-push-start"
import { startSafePush } from "@/lib/safe-push-start"
import { stackAncestors, stackPushOrder } from "@/lib/stack"
import { showToast } from "@/lib/toast-message"
import { branchSyncStore, getBranch, stackDescendantsIn } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getBranchRobot, getPatronusGitHubRepository, getPushHold, isPatronusAvailable, patronusRequest, patronusStore } from "@/stores/patronus"
import { getPullRequest, getPullRequestAvailability, pullRequestsStore } from "@/stores/pull-requests"
import type { ReadinessEntry } from "@/stores/review-readiness"
import { demandReviewReadiness, freshReadiness, getReadinessTarget, getReviewReadiness, reviewReadinessStore, useDemandReviewReadiness } from "@/stores/review-readiness"
import type { SafePushAsk, SafePushMode, SafePushPlacement } from "@/stores/safe-push"
import { getSafePushStarting, openSafePushPopover, safePushStore, setSafePushStarting } from "@/stores/safe-push"
import { getShownBranchTableOrder } from "@/stores/stack-collapse"

/** How long a click waits for the facts its decision needs (the review, the rules) before the popover says what it waits for */
export const SAFE_PUSH_SETTLE_MS = 20_000
/** The merge profiles of a target are asked again after this: Patronus caches the answer for the app (10 minutes) */
const START_OPTIONS_TTL_MS = 60_000

/** What asks a start: its mode, how (a click, a key), where its popover opens, a profile chosen where it was asked */
export interface SafePushRequest {
  readonly mode: SafePushMode
  readonly via: "click" | "key"
  readonly placement?: SafePushPlacement
  /** A profile chosen in the menu (`Safe Push With ›`); undefined: the branch's own (its last robot's) */
  readonly profileId?: string | null
  /** A Safe Push without review, asked from the Create Pull Request dialog */
  readonly bypassReview?: boolean
}

/** A start as the stores have it now: the facts, the decision, the start it would run, and what its popover shows */
export interface SafePushRead {
  readonly facts: SafePushFacts
  readonly decision: SafePushDecision
  readonly start: SafePushStart
  /** The branches it is built on, root first: its Safe Push lands them with it */
  readonly ancestors: readonly string[]
  /** The branches built on it: they stay */
  readonly children: readonly string[]
  /** The commits it lands: its own and its ancestors' */
  readonly commitCount: number
  /** The branches its start pushes, root first */
  readonly pushOrder: readonly string[]
  /** The profiles the mode allows; for a start without review, in groups (`bypassProfiles`) */
  readonly profiles: readonly PatronusProfile[]
  readonly bypassProfiles: BypassProfiles | null
  /** The profile of the branch's last robot, if still offered: the profile a start runs with unless another is chosen */
  readonly branchProfileId: string | null
  /** The review of the branch as Patronus answered it for the branch as it is; null while it loads, for a dry run */
  readonly readiness: ReadinessEntry | null
}

// ---- the merge profiles ----

interface OptionsEntry {
  readonly at: number
  readonly promise: Promise<StartOptionsFact>
  value: StartOptionsFact | null
}

const optionsCache = new Map<string, OptionsEntry>()

/** By repository, baseline and the branch's last robot (whose profile is the default) */
function optionsKey(branchName: string): string {
  const { repositoryPath } = patronusRequest()
  return JSON.stringify([repositoryPath, branchSyncStore.getState().baselineBranch ?? "", getBranchRobot(branchName)?.id ?? null])
}

/** The target and the merge profiles of a start of the branch (`get_patronus_start_options`), asked once a minute */
export function loadStartOptions(branchName: string): Promise<StartOptionsFact> {
  const key = optionsKey(branchName)
  const cached = optionsCache.get(key)
  if (cached !== undefined && Date.now() - cached.at < START_OPTIONS_TTL_MS && cached.value?.status !== "error") {
    return cached.promise
  }
  const entry: OptionsEntry = {
    at: Date.now(),
    value: null,
    promise: commands
      .getPatronusStartOptions({
        repositoryPath: patronusRequest().repositoryPath,
        baselineBranch: branchSyncStore.getState().baselineBranch ?? "",
        lastRobotId: getBranchRobot(branchName)?.id ?? null,
      })
      .then(
        (result): StartOptionsFact => (result.status === "ok" ? { status: "loaded", options: result.data } : { status: "error", message: result.error.message }),
        (error: unknown): StartOptionsFact => ({ status: "error", message: error instanceof Error ? error.message : String(error) }),
      )
      .then(value => {
        entry.value = value
        return value
      }),
  }
  optionsCache.set(key, entry)
  return entry.promise
}

/** The answer of `loadStartOptions` while the popover shows: loading until it arrives */
export function useStartOptions(branchName: string): StartOptionsFact {
  const [state, setState] = useState<{ branchName: string; value: StartOptionsFact } | null>(null)
  useEffect(() => {
    let current = true
    void loadStartOptions(branchName).then(value => {
      if (current) {
        setState({ branchName, value })
      }
    })
    return () => {
      current = false
    }
  }, [branchName])
  return state?.branchName === branchName ? state.value : { status: "loading" }
}

/** Forgets the profiles asked (tests) */
export function resetStartOptions(): void {
  optionsCache.clear()
}

// ---- the facts ----

/** The code-owner review of a Safe Push as `getSafePushDecision` reads it */
function reviewFactOf(checksReview: boolean, entry: ReadinessEntry | null): ReviewFact {
  if (!checksReview) {
    return { kind: "none" }
  }
  if (getPatronusGitHubRepository() === null) {
    // a repository only on Space: its merge request counts, the robot decides
    if (entry === null) {
      return { kind: "loading" }
    }
    return entry.requirement === null ? { kind: "loaded", readiness: entry.readiness } : { kind: "space", required: entry.requirement.required }
  }
  // until GitHub lists the pull requests the start is not known (no token, SSO): the robot decides
  if (getPullRequestAvailability()?.status !== "available") {
    return { kind: "unlisted" }
  }
  return entry === null ? { kind: "loading" } : { kind: "loaded", readiness: entry.readiness }
}

/**
 * A start of the branch as the stores have it now (`SafePushRead`), with the merge profiles as loaded; null without the
 * branch. The one reader of the facts: the click decides on it, the popover renders it.
 */
export function readSafePush(branchName: string, ask: Omit<SafePushRequest, "placement">, options: StartOptionsFact): SafePushRead | null {
  const branch = getBranch(branchName)
  if (branch === undefined) {
    return null
  }
  const { mode } = ask
  const localTip = branch.commits[0]?.hash
  const remoteStatus = branch.remoteStatus
  // a Safe Push of the branch running on an older commit: the start replaces it, the push before it cancels it
  const hold = getPushHold(branchName)
  const replaces = mode === "rebaseAutosquash" && hold?.kind === "own" && !isStartBlocked(hold.robot, localTip, remoteStatus) ? hold.robot : null
  const isHeld = (name: string): boolean => {
    const it = getPushHold(name)
    return it !== null && it.robot.id !== replaces?.id
  }
  // the branches the start pushes, root first: the stack while an ancestor has work for the remote, up to a held one
  const pushOrder = stackPushOrder(branchName, getBranch, { stopAtConfirm: true, isHeld })
  const startPush = getStartPush(getPushAction({ remoteStatus }), pushOrder.length > 1)
  const needsPush = startPush.kind === "stack" || startPush.kind === "branch"
  const step = getBranchNextStepOf(branch, { forStart: true })

  const loaded = options.status === "loaded" ? options.options : null
  const target = loaded?.targetBranch ?? getReadinessTarget()
  const pullRequest = getPullRequest(branchName)
  const fromPullRequest = getPullRequestStart(getPatronusGitHubRepository(), pullRequest, mode, target)
  const profiles = loaded?.profiles.filter(profile => allowsPushMode(profile, mode)) ?? []
  const branchProfileId = profiles.some(profile => profile.id === loaded?.defaultProfileId) ? (loaded?.defaultProfileId ?? null) : null

  // the code owners don't check a dry run; the approvals of the pull request count only for the robot started from it
  const checksReview = mode === "rebaseAutosquash" && loaded?.supported === true
  const readiness = checksReview ? freshReadiness(getReviewReadiness(branchName), branchName) : null
  const review = reviewFactOf(checksReview, readiness)
  // without review only while no pull request is open (a comment can't choose a profile) and the approvals are missing
  const bypassReview = ask.bypassReview === true && mode === "rebaseAutosquash" && fromPullRequest === null && !(review.kind === "loaded" && review.readiness.kind !== "missing")
  const bypassProfiles = loaded === null || !bypassReview ? null : getBypassProfiles(loaded, mode)
  const offered = bypassProfiles === null ? profiles : [...bypassProfiles.skipping, ...bypassProfiles.other, ...bypassProfiles.emergency]
  const profileId = ask.profileId !== undefined ? ask.profileId : bypassProfiles === null ? branchProfileId : bypassProfiles.defaultProfileId
  // a comment on the pull request runs the default checks of the target
  const profile = fromPullRequest === null ? (offered.find(it => it.id === profileId) ?? null) : null

  // a push of new commits into a pull request someone approved: the rules of the repository may dismiss the approvals
  const approvedPullRequest = needsPush && fromPullRequest !== null && pullRequest?.reviewers.some(reviewer => reviewer.state === "approved") === true
  const dismiss = approvedPullRequest ? { pullRequest: fromPullRequest.number, dismisses: peekDismissesStaleReviews(patronusRequest().repositoryPath, target) } : null

  const state = branchSyncStore.getState()
  const ancestors = stackAncestors(branchName, getBranch).map(it => it.name)
  const facts: SafePushFacts = {
    branchName,
    mode,
    via: ask.via,
    step: { kind: step.kind, blocker: mode === "dryRun" ? step.dryRunBlocker : step.safePushBlocker },
    target,
    options,
    profile,
    bypassReview,
    fromPullRequest,
    startPush,
    hasTip: localTip !== undefined,
    replaces: replaces === null ? null : (replaces.sourceRevision ?? ""),
    pushHeld: needsPush && hold !== null && replaces === null ? formatPushHoldReason(branchName, hold) : null,
    review,
    dismiss,
    ancestorCount: ancestors.length,
  }
  return {
    facts,
    decision: getSafePushDecision(facts),
    start: {
      branchName,
      mode,
      profile: profile === null ? null : { id: profile.id, name: profile.name },
      fromPullRequest,
      mergeRequest: readiness?.requirement?.mergeRequest ?? null,
      needsPush,
      replaces,
    },
    ancestors,
    children: stackDescendantsIn(state, branchName),
    commitCount: [...ancestors, branchName].reduce((sum, name) => sum + (state.branches[name]?.commitCount ?? 0), 0),
    pushOrder,
    profiles,
    bypassProfiles,
    branchProfileId,
    readiness,
  }
}

/** Resolves on the next change of a store the facts are read from, or after `ms` */
function nextChange(ms: number): Promise<void> {
  return new Promise(resolve => {
    const stores = [branchSyncStore, reviewReadinessStore, pullRequestsStore, patronusStore]
    let unsubscribes: (() => void)[] = []
    const done = () => {
      clearTimeout(timer)
      unsubscribes.forEach(unsubscribe => unsubscribe())
      unsubscribes = []
      resolve()
    }
    const timer = setTimeout(done, ms)
    unsubscribes = stores.map(store => store.subscribe(done))
  })
}

/** The facts of a click, read again on every change until the decision waits for nothing, at most `SAFE_PUSH_SETTLE_MS` */
async function settleSafePush(branchName: string, ask: Omit<SafePushRequest, "placement">): Promise<SafePushRead | null> {
  const options = await loadStartOptions(branchName)
  const deadline = Date.now() + SAFE_PUSH_SETTLE_MS
  for (;;) {
    const read = readSafePush(branchName, ask, options)
    if (read === null || read.decision.kind !== "wait") {
      return read
    }
    const left = deadline - Date.now()
    if (left <= 0) {
      return read
    }
    const dismiss = read.facts.dismiss
    await (dismiss?.dismisses === null ? Promise.race([readDismissesStaleReviews(patronusRequest().repositoryPath, read.facts.target), nextChange(left)]) : nextChange(left))
  }
}

/** A start that can't run: the reason as an `info` toast (a precondition); the push alone where it is the step */
function tellRefusal(branchName: string, mode: SafePushMode, reason: string, offersPush: boolean): void {
  showToast({
    type: "info",
    title: `${formatPushMode(mode)} of ${branchName} can't start`,
    description: reason,
    ...(offersPush && {
      action: {
        children: "Push",
        onClick: () =>
          void pushForAction(branchName).then(result => {
            if (!result.pushed) {
              notifyError(`Cannot push ${branchName}`, result.reason)
            }
          }),
      },
    }),
  })
}

/**
 * A start of a Safe Push or a Dry Run, asked from any surface: the slot of the row or of the island header, the branch
 * menu, S, the preview, the review window, a toast. What every surface knows refuses it first, as the step says it (an
 * `info` toast). S opens the popover at once, which reads the facts as they load. A click waits for the facts, its
 * button busy ("Checking…"), and then starts at once when there is nothing to say (`getSafePushDecision`), else opens
 * the popover at the slot of the branch (a hidden row revealed first), or says why it can't start.
 */
export async function requestSafePush(branchName: string, request: SafePushRequest): Promise<void> {
  const branch = getBranch(branchName)
  if (branch === undefined || !isPatronusAvailable() || getSafePushStarting(branchName) !== null) {
    return
  }
  const { mode } = request
  const placement = request.placement ?? "row"
  const step = getBranchNextStepOf(branch)
  // a start without review is held back only by the reasons of the branch, never by the approvals it goes without
  const blocker = mode === "dryRun" ? step.dryRunBlocker : request.bypassReview === true ? (step.withoutReview?.blocker ?? null) : step.safePushBlocker
  if (blocker !== null) {
    if (mode === "rebaseAutosquash" && step.notice !== null) {
      // a step that waits in the pull request (the approvals, a draft, another base) opens it from the notice
      const pullRequest = getPullRequest(branchName)
      const opensPullRequest = pullRequest !== undefined && step.tone !== "default"
      showToast({
        type: "info",
        title: step.notice.title,
        description: step.notice.description,
        ...(opensPullRequest && { action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } }),
      })
    } else {
      tellRefusal(branchName, mode, blocker, false)
    }
    return
  }
  // the popover and the progress show at the row's slot: a row a collapsed stack or the filter hides is revealed first
  if (placement === "row" && !getShownBranchTableOrder().includes(branchName)) {
    requestReveal({ branchName, commitIds: [] })
  }
  const ask: SafePushAsk = {
    branchName,
    mode,
    placement,
    ...(request.profileId !== undefined && { profileId: request.profileId }),
    ...(request.bypassReview === true && { bypassReview: true }),
  }
  if (request.via === "key" || request.bypassReview === true) {
    openSafePushPopover(ask)
    return
  }
  setSafePushStarting(branchName, { mode, step: "checking" })
  const release = mode === "rebaseAutosquash" ? demandReviewReadiness(branchName) : () => {}
  let read: SafePushRead | null
  try {
    read = await settleSafePush(branchName, request)
  } finally {
    release()
    if (getSafePushStarting(branchName)?.step === "checking") {
      setSafePushStarting(branchName, null)
    }
  }
  if (read === null) {
    return
  }
  const { decision } = read
  switch (decision.kind) {
    case "start":
      await startSafePush(read.start)
      return
    case "refuse":
      tellRefusal(branchName, mode, decision.reason, decision.offersPush)
      return
    case "wait":
    case "choose":
    case "confirm":
      openSafePushPopover(ask)
  }
}

/** The read the popover shows, and what it was read from: the same object while nothing it reads changed */
let shownRead: { inputs: readonly unknown[]; read: SafePushRead | null } | null = null

/**
 * `readSafePush` of the popover, once per change of what it reads (the stores, the profiles, the rules asked): the
 * subscriptions of the popover share the answer, and a selector returns the same object for the same state
 */
function readSafePushNow(branchName: string, ask: Omit<SafePushRequest, "placement">, options: StartOptionsFact, rules: number): SafePushRead | null {
  const inputs = [
    branchSyncStore.getState(),
    reviewReadinessStore.getState(),
    pullRequestsStore.getState(),
    patronusStore.getState(),
    safePushStore.getState(),
    branchName,
    ask.mode,
    ask.via,
    ask.profileId,
    ask.bypassReview,
    options,
    rules,
  ]
  if (shownRead !== null && shownRead.inputs.every((input, index) => input === inputs[index])) {
    return shownRead.read
  }
  const read = readSafePush(branchName, ask, options)
  shownRead = { inputs, read }
  return read
}

/** `readSafePush` of the popover's ask, read again on every change of the stores it reads; the review demanded meanwhile */
export function useSafePushRead(ask: SafePushAsk, profileId: string | null | undefined): SafePushRead | null {
  const options = useStartOptions(ask.branchName)
  useDemandReviewReadiness(ask.branchName, ask.mode === "rebaseAutosquash")
  // the rules of the repository, asked while the start would push into an approved pull request: an answer reads again
  const [rules, setRules] = useState(0)
  const request: Omit<SafePushRequest, "placement"> = { mode: ask.mode, via: "key", bypassReview: ask.bypassReview, ...(profileId !== undefined && { profileId }) }
  // the read flows out of the subscriptions: a value read beside them would be memoized by the compiler
  const select = () => readSafePushNow(ask.branchName, request, options, rules)
  useStore(branchSyncStore, select)
  useStore(reviewReadinessStore, select)
  useStore(pullRequestsStore, select)
  useStore(patronusStore, select)
  const read = useStore(safePushStore, select)
  const dismissPending = read?.facts.dismiss?.dismisses === null
  const target = read?.facts.target ?? ""
  useEffect(() => {
    if (!dismissPending) {
      return
    }
    let current = true
    void readDismissesStaleReviews(patronusRequest().repositoryPath, target).then(() => {
      if (current) {
        setRules(count => count + 1)
      }
    })
    return () => {
      current = false
    }
  }, [dismissPending, target])
  return read
}
