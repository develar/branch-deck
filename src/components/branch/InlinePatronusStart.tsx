import { useEffect, useState } from "react"
import { ArrowDownToLineIcon, FlaskConicalIcon, Layers2Icon, ShieldCheckIcon, TriangleAlertIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { PatronusReviewBlock } from "@/components/branch/PatronusReviewBlock"
import { InlineForm } from "@/components/shared/InlineForm"
import { useDismissesStaleReviews } from "@/hooks/useDismissesStaleReviews"
import { moveCommits } from "@/hooks/useMoveCommits"
import { usePatronusReviewRequirement } from "@/hooks/usePatronusReviewRequirement"
import { restackBranch } from "@/hooks/useStackActions"
import { useStackIndependence } from "@/hooks/useStackIndependence"
import type { Commit, PatronusPushMode, PatronusRobot, PatronusStartOptions, StackCommit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { canCreatePullRequest } from "@/lib/git/new-pull-request"
import { cancelHoldingRobot, pushBranch, pushStack } from "@/lib/git/push"
import { formatShortHash } from "@/lib/hash"
import { allowsPushMode, formatPushHoldReason, formatPushMode, getPullRequestStart, getStartPush, isStartBlocked } from "@/lib/patronus"
import { announceRobot } from "@/lib/patronus-actions"
import { describeStackStart } from "@/lib/patronus-stack"
import type { StackLead } from "@/lib/patronus-stack"
import { getPushAction } from "@/lib/push-summary"
import { stackAncestors, stackPushOrder } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import { transferLabel } from "@/lib/transfer-progress"
import {
  branchSyncStore,
  getBranch,
  stackDescendantsIn,
  useBranchPushProgress,
  useBranchRemoteStatus,
  useBranchSync,
  useBranchTip,
  useStackPushCurrent,
} from "@/stores/branch-sync"
import { openCreatePullRequestDialog } from "@/stores/create-pull-request-dialog"
import { closeInline, openInline, setInlinePushing, useIsRowProcessing, withRowProgress } from "@/stores/inline-row-action"
import {
  getBranchRobot,
  patronusRequest,
  startPatronusFromPullRequest,
  startPatronusRobot,
  useBranchRobot,
  useLandingAncestor,
  useLandingSpan,
  usePatronusGitHubRepository,
  usePushHold,
  usePushHolds,
} from "@/stores/patronus"
import { getPullRequest, usePullRequest } from "@/stores/pull-requests"
import { pinStackPath } from "@/stores/stack-view"

/** The Select value of "no profile": Patronus runs the default checks of the target branch */
const NO_PROFILE = "\0none"

type StartOptionsState = { status: "loading" } | { status: "loaded"; options: PatronusStartOptions } | { status: "error"; message: string }

/** Names, one per line, as a selector returns them so the store notifies only when they change */
function splitNames(names: string): string[] {
  return names === "" ? [] : names.split("\n")
}

/**
 * The commits of the branch list that `commits` name, for a move: each in its branch, by its original hash. The form
 * offers the move only while the list has every one of them
 */
function syncedCommits(commits: readonly StackCommit[]): Commit[] {
  return commits.flatMap(commit => {
    const branch = getBranch(commit.branch)
    const index = branch?.commitIndex[commit.id]
    const synced = index === undefined ? undefined : branch?.commits[index]
    return synced === undefined ? [] : [synced]
  })
}

/** The target branch and the merge profiles, asked once when the form opens (Patronus caches nothing for the app) */
function useStartOptions(branchName: string): StartOptionsState {
  const [state, setState] = useState<StartOptionsState>({ status: "loading" })
  useEffect(() => {
    let current = true
    commands
      .getPatronusStartOptions({
        repositoryPath: patronusRequest().repositoryPath,
        baselineBranch: branchSyncStore.getState().baselineBranch ?? "",
        lastRobotId: getBranchRobot(branchName)?.id ?? null,
      })
      .then(
        result => {
          if (current) {
            setState(result.status === "ok" ? { status: "loaded", options: result.data } : { status: "error", message: result.error.message })
          }
        },
        (error: unknown) => {
          if (current) {
            setState({ status: "error", message: error instanceof Error ? error.message : String(error) })
          }
        },
      )
    return () => {
      current = false
    }
  }, [branchName])
  return state
}

/**
 * Starts a Safe Push (`rebaseAutosquash`) or a Dry Run of the branch: the merge profile (the one of the branch's last
 * robot by default), what will happen, and Start. A branch whose commits the remote lacks, by content as the push
 * button decides it, is pushed first (`getStartPush`); one only rebuilt on a newer baseline is not, Patronus rebases the
 * remote branch itself. Start waits for the remote status. A branch with an open pull request in the GitHub repository
 * the branches are mirrored to starts from it, a `/patronus` comment: then the approvals of the pull request count and
 * Patronus closes it once merged; a comment can't choose a profile. A push that dismisses its approvals (the rules of
 * the repository dismiss stale reviews) is told before Start. On GitHub, Start waits for the readiness of the branch as
 * it is (`usePatronusReviewRequirement`, never a stale answer) and is disabled with the reason while Patronus would miss
 * approvals: it checks them when the robot starts and fails without them. A dry run is never checked.
 *
 * A stacked branch carries the commits of the branches it is built on: its Safe Push lands them too. The form leads root
 * first (a Safe Push of the root-most ancestor, whose merge moves the branch onto the target), and Start lands the stack
 * in one robot: the ancestors with work for the remote are pushed first, and a pull request into the parent's branch
 * isn't the start (Patronus merges a pull request into its base), the API is. A Safe Push of a parent says what its
 * merge does to the branches stacked on it.
 */
export function InlinePatronusStart({ branchName, mode }: { branchName: string; mode: Extract<PatronusPushMode, "rebaseAutosquash" | "dryRun"> }) {
  const state = useStartOptions(branchName)
  const processing = useIsRowProcessing(branchName)
  const localTip = useBranchTip(branchName)
  const remoteStatus = useBranchRemoteStatus(branchName)
  const robot = useBranchRobot(branchName)
  const githubRepository = usePatronusGitHubRepository()
  const pullRequest = usePullRequest(branchName)
  const [chosenProfile, setChosenProfile] = useState<string | null>(null)
  // the action whose first step pushes the branch: its button shows the push ("Pushing 45%"), the row's doesn't
  const [pushingFor, setPushingFor] = useState<"start" | "pullRequest" | null>(null)
  const isPushing = pushingFor !== null
  const stackPushCurrent = useStackPushCurrent(branchName)
  const pushProgress = useBranchPushProgress(stackPushCurrent ?? branchName)
  const pushLabel = transferLabel(stackPushCurrent === null || stackPushCurrent === branchName ? "Pushing" : `Pushing ${stackPushCurrent}`, pushProgress)
  const ancestors = splitNames(
    useBranchSync(state =>
      stackAncestors(branchName, name => state.branches[name])
        .map(it => it.name)
        .join("\n"),
    ),
  )
  const children = splitNames(useBranchSync(state => stackDescendantsIn(state, branchName).join("\n")))
  const ancestorCommitCount = useBranchSync(state => ancestors.reduce((sum, name) => sum + (state.branches[name]?.commitCount ?? 0), 0))
  // a Safe Push of the branch running on an older commit: the start replaces it, the push before it cancels it
  const hold = usePushHold(branchName)
  const replaces = mode === "rebaseAutosquash" && hold?.kind === "own" && !isStartBlocked(hold.robot, localTip, remoteStatus) ? hold.robot : null
  const holds = usePushHolds()
  // the branches the start pushes, root first: the stack while an ancestor has work for the remote, up to a held one
  const isHeld = (name: string): boolean => {
    const it = holds.get(name)
    return it !== undefined && it.robot.id !== replaces?.id
  }
  const pushOrder = splitNames(useBranchSync(state => stackPushOrder(branchName, name => state.branches[name], { stopAtConfirm: true, isHeld }).join("\n")))
  // the stack graph shows what the start lands while the form is open
  useEffect(() => {
    pinStackPath(branchName, true)
    return () => pinStackPath(branchName, false)
  }, [branchName])
  // a stacked branch may build alone: the lead offers the unstack, or the move of what it needs, before root first
  const independence = useStackIndependence(branchName, mode === "rebaseAutosquash" && ancestors.length > 0)
  const answered = independence.state.status === "loaded" ? independence.state.independence : null
  const neededCommits = answered?.kind === "needs" ? answered.parentCommits : []
  // the check reads HEAD, the branch list the last sync: a move takes what the list has, so it waits for every commit
  const neededInList = useBranchSync(state => neededCommits.every(commit => state.branches[commit.branch]?.commitIndex[commit.id] !== undefined))
  const [isRegrouping, setRegrouping] = useState(false)
  const landingAncestor = useLandingAncestor(branchName)
  const landingSpan = useLandingSpan(branchName)

  const modeName = formatPushMode(mode)
  const options = state.status === "loaded" ? state.options : null
  const fromPullRequest = getPullRequestStart(githubRepository, pullRequest, mode, options?.targetBranch ?? null)
  const profiles = options?.profiles.filter(profile => allowsPushMode(profile, mode)) ?? []
  const defaultProfile = profiles.some(profile => profile.id === options?.defaultProfileId) ? options!.defaultProfileId! : NO_PROFILE
  const profile = chosenProfile ?? defaultProfile
  const startPush = getStartPush(getPushAction({ remoteStatus }), pushOrder.length > 1)
  const pushesStack = startPush.kind === "stack"
  const needsPush = pushesStack || startPush.kind === "branch"
  const blocked = isStartBlocked(robot, localTip, remoteStatus)
  // a dry run pushes nothing into the target: it may run while an ancestor lands, or while a descendant lands the branch
  const waitsForAncestor = mode === "rebaseAutosquash" ? landingAncestor : null
  const landsWithDescendant = mode === "rebaseAutosquash" ? landingSpan : null
  // a dry run that needs a push into a branch a Safe Push holds: the push won't land, and it would break the Safe Push
  const pushHeld = needsPush && hold !== null && replaces === null && !blocked && waitsForAncestor === null && landsWithDescendant === null ? hold : null
  // the code owners don't check a dry run; the approvals of the pull request count only for the robot started from it
  const checksReview = mode === "rebaseAutosquash" && options?.supported === true
  const review = usePatronusReviewRequirement(branchName, checksReview)
  const mergeRequest = review.state.status === "loaded" ? review.state.requirement.mergeRequest : null
  // on GitHub the start waits for the answer for the branch as it is, and Patronus would fail it without the approvals;
  // a repository only on Space keeps the start (its merge requests are deprecated, the robot decides)
  const reviewOnGitHub = checksReview && githubRepository !== null
  const reviewLoading = reviewOnGitHub && review.state.status === "loading"
  const approvalsMissing = reviewOnGitHub && review.state.status === "loaded" && review.state.readiness.kind === "missing"
  const canStart =
    options !== null &&
    options.supported &&
    !blocked &&
    waitsForAncestor === null &&
    landsWithDescendant === null &&
    pushHeld === null &&
    !processing &&
    !isPushing &&
    localTip !== undefined &&
    startPush.kind !== "unknown" &&
    fromPullRequest?.blockedByDraft !== true &&
    !reviewLoading &&
    !approvalsMissing
  const submitBlocker = approvalsMissing
    ? "Patronus checks the approvals when the robot starts and fails without them"
    : fromPullRequest?.blockedByDraft === true
      ? `#${fromPullRequest.number} is a draft: Patronus merges it once it is ready for review`
      : null
  // a push of new commits into a pull request someone approved: the rules of the repository may dismiss the approvals
  const approvedPullRequest = needsPush && fromPullRequest !== null && pullRequest?.reviewers.some(reviewer => reviewer.state === "approved") === true
  const dismissesApprovals = useDismissesStaleReviews(options?.targetBranch ?? null, approvedPullRequest)

  /**
   * The branch on the remote with its commits: a start and a merge request need it. False when the push failed (it said
   * so). A Safe Push running on an older commit is canceled for the push of a start (`replacesRobot`), not restarted:
   * the form starts the new one; without a push it is canceled for the start alone. A dry run doesn't replace it: the
   * form offers no start while it needs a push then. The push is a step of the form's action: the start's toast or the
   * opened pull request say how it went, not a push toast.
   */
  const pushIfNeeded = async (purpose: "start" | "pullRequest", replacesRobot: PatronusRobot | undefined): Promise<boolean> => {
    if (!needsPush) {
      return replacesRobot === undefined || (await cancelHoldingRobot(replacesRobot))
    }
    setPushingFor(purpose)
    setInlinePushing(branchName, true)
    const pushed = pushesStack
      ? await pushStack(branchName, { stopAtConfirm: true, replacesRobot, announce: false })
      : await pushBranch(branchName, { replacesRobot, announce: false })
    setInlinePushing(branchName, false)
    setPushingFor(null)
    return pushed
  }

  const start = async () => {
    if (!(await pushIfNeeded("start", replaces ?? undefined))) {
      return
    }
    if (fromPullRequest !== null) {
      await withRowProgress(
        branchName,
        async () => {
          try {
            announceRobot(`${modeName} started`, await startPatronusFromPullRequest(fromPullRequest.number, mode))
            return true
          } catch (error) {
            notifyError(`Cannot start the ${modeName}`, error)
            return false
          }
        },
        { processingMessage: `Asking Patronus on pull request #${fromPullRequest.number}…` },
      )
      return
    }
    const profileId = profile === NO_PROFILE ? null : profile
    const profileName = profiles.find(it => it.id === profileId)?.name ?? null
    await withRowProgress(
      branchName,
      async () => {
        try {
          announceRobot(`${modeName} started`, await startPatronusRobot(branchName, mode, profileId, mergeRequest, profileName), profileName)
          return true
        } catch (error) {
          notifyError(`Cannot start the ${modeName}`, error)
          return false
        }
      },
      { processingMessage: `Starting the ${modeName}…` },
    )
  }

  /**
   * The branch on the remote with its tip, then the dialog of its pull request, where the approvals count; once it is
   * created, the form starts from it (`fromPullRequest`)
   */
  const createPullRequest = async () => {
    // the pull request doesn't replace a running Safe Push: its push waits for it as any push
    if (!(await pushIfNeeded("pullRequest", undefined))) {
      return
    }
    const branch = getBranch(branchName)
    if (branch !== undefined && canCreatePullRequest(branch)) {
      openCreatePullRequestDialog(branchName)
    }
  }

  const target = options?.targetBranch ?? "the baseline"
  const robotWork =
    mode === "dryRun"
      ? `Patronus rebases ${branchName} onto ${target} and runs the checks, without pushing into ${target}.`
      : `Patronus rebases ${branchName} onto ${target}, runs the checks and pushes it into ${target} when they pass.`
  const explanation =
    fromPullRequest === null
      ? robotWork
      : `Comments ${fromPullRequest.command} on pull request #${fromPullRequest.number}: ${robotWork} It reports in the pull request${mode === "dryRun" ? "" : " and closes it once merged"}.`
  // a dry run doesn't push into the target, but Patronus starts from the remote branch: the branch goes there first
  const unpushed = startPush.kind === "branch" && startPush.action === "create" ? `${branchName} isn't on the remote yet` : `The remote branch of ${branchName} isn't up to date`
  const pushNote = pushesStack
    ? `${joinNames(pushOrder)} have commits the remote lacks: they are pushed first, parents first, then Patronus starts from the remote branch of ${branchName}.`
    : `${unpushed}: it is pushed first, then Patronus starts from the remote branch.`
  const stackNotes = describeStackStart({
    branchName,
    mode,
    target,
    ancestors,
    ancestorCommitCount,
    children,
    // a pull request the start doesn't comment on: its approvals don't count
    pullRequest: pullRequest?.state === "open" && fromPullRequest === null && githubRepository !== null ? pullRequest.number : null,
    childrenHavePullRequests: children.some(child => getPullRequest(child)?.state === "open"),
    independence: answered?.kind === "needs" && !neededInList ? { ...answered, movable: false } : answered,
  })

  /** The unstack, awaited with its sync: the form shows itself again with the review of the branch alone, no start */
  const unstack = async () => {
    setRegrouping(true)
    await restackBranch(branchName, null, { awaitSync: true })
    setRegrouping(false)
  }

  /** The move of the needed commits into the branch, then the check again */
  const moveHere = async (commits: readonly StackCommit[]) => {
    setRegrouping(true)
    if (await moveCommits(syncedCommits(commits), branchName)) {
      independence.recheck()
    }
    setRegrouping(false)
  }
  const reviewBlock = (
    <PatronusReviewBlock
      branchName={branchName}
      review={review}
      github={githubRepository === null ? null : { pullRequest, target }}
      onCreatePullRequest={createPullRequest}
      pushLabel={pushingFor === "pullRequest" ? pushLabel : null}
    />
  )
  const items = [{ value: NO_PROFILE, label: "No profile" }, ...profiles.map(it => ({ value: it.id, label: it.name }))]
  return (
    <InlineForm
      title={`${modeName} of ${branchName}`}
      // with the approvals missing, the review is the point: it comes first, what the start would do after it
      description={approvalsMissing ? undefined : <span data-testid="patronus-start-explanation">{explanation}</span>}
      dataTestId="patronus-start-form"
      canSubmit={canStart}
      submitDisabledReason={submitBlocker}
      loading={processing || pushingFor === "start" || isRegrouping}
      submitText={pushingFor === "start" ? pushLabel : needsPush ? `${pushesStack ? "Push Stack" : "Push"} and Start ${modeName}` : `Start ${modeName}`}
      submitIcon={mode === "dryRun" ? <FlaskConicalIcon data-icon="inline-start" /> : <ShieldCheckIcon data-icon="inline-start" />}
      // a Safe Push of a stacked branch lands its ancestors too: Enter doesn't choose it, the lead is root first
      focusSubmit={stackNotes.lead === null}
      onCancel={closeInline}
      onSubmit={() => void start()}
    >
      {approvalsMissing && (
        <>
          {reviewBlock}
          <p className="text-sm text-muted-foreground" data-testid="patronus-start-explanation">
            {explanation}
          </p>
        </>
      )}
      {stackNotes.lead !== null && (
        <StackLeadBlock
          lead={stackNotes.lead}
          checking={independence.state.status === "loading"}
          branchName={branchName}
          disabled={processing || isPushing || isRegrouping}
          onUnstack={() => void unstack()}
          onMove={commits => void moveHere(commits)}
        />
      )}
      {stackNotes.note !== null && (
        <p className="text-sm text-muted-foreground" data-testid="patronus-start-stack-note">
          {stackNotes.note}
        </p>
      )}
      {waitsForAncestor !== null && (
        <p className="flex items-start gap-1.5 text-sm text-warning" data-testid="patronus-start-landing-ancestor">
          <TriangleAlertIcon className="mt-0.5 size-4 shrink-0" />
          {`A ${formatPushMode(waitsForAncestor.robot.pushMode)} of ${waitsForAncestor.branch} is running: it lands ${waitsForAncestor.branch}'s commits, which ${branchName} carries. Start once it merged.`}
        </p>
      )}
      {landsWithDescendant !== null && (
        <p className="flex items-start gap-1.5 text-sm text-warning" data-testid="patronus-start-landing-descendant">
          <TriangleAlertIcon className="mt-0.5 size-4 shrink-0" />
          {`A ${formatPushMode(landsWithDescendant.robot.pushMode)} of ${landsWithDescendant.branch} is running: ${landsWithDescendant.branch} is built on ${branchName}, so it pushes ${branchName}'s commits into ${target} too.`}
        </p>
      )}
      {replaces !== null && (
        <p className="flex items-start gap-1.5 text-sm text-warning" data-testid="patronus-start-replaces">
          <TriangleAlertIcon className="mt-0.5 size-4 shrink-0" />
          {`A ${modeName} of ${branchName} runs on ${formatShortHash(replaces.sourceRevision ?? "")}: the start cancels it.`}
        </p>
      )}
      {pushHeld !== null && (
        <p className="flex items-start gap-1.5 text-sm text-warning" data-testid="patronus-start-push-held">
          <TriangleAlertIcon className="mt-0.5 size-4 shrink-0" />
          {`${formatPushHoldReason(branchName, pushHeld)}. The ${modeName} needs a push of ${branchName} first.`}
        </p>
      )}
      {state.status === "loading" && (
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <Spinner className="size-4" />
          Asking Patronus for the merge profiles…
        </p>
      )}
      {state.status === "error" && (
        <p className="text-sm text-destructive" data-testid="patronus-start-error">
          {state.message}
        </p>
      )}
      {options !== null && !options.supported && (
        <p className="text-sm text-destructive" data-testid="patronus-start-error">{`Patronus does not push into ${options.targetBranch}.`}</p>
      )}
      {options !== null && options.supported && fromPullRequest !== null && (
        <p className="text-sm text-muted-foreground" data-testid="patronus-start-profile-note">
          A pull request comment starts the default checks of the target branch: it can't choose a merge profile.
        </p>
      )}
      {fromPullRequest?.blockedByDraft === true && (
        <p className="flex items-start gap-1.5 text-sm text-warning" data-testid="patronus-start-draft">
          <TriangleAlertIcon className="mt-0.5 size-4 shrink-0" />
          {`Pull request #${fromPullRequest.number} is a draft: Patronus merges it once it is ready for review.`}
        </p>
      )}
      {options !== null && options.supported && fromPullRequest === null && (
        <label className="flex items-center gap-3 text-sm">
          <span className="text-muted-foreground">Profile</span>
          <Select items={items} value={profile} onValueChange={value => setChosenProfile(value)} disabled={processing}>
            <SelectTrigger size="sm" className="w-64" data-testid="patronus-profile">
              <SelectValue />
            </SelectTrigger>
            <SelectContent alignItemWithTrigger={false}>
              {items.map(item => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </label>
      )}
      {checksReview && !approvalsMissing && reviewBlock}
      {needsPush && (
        <p className="text-sm text-muted-foreground" data-testid="patronus-start-push-note">
          {pushNote}
        </p>
      )}
      {startPush.kind === "none" && remoteStatus?.remoteTip !== localTip && (
        <p className="text-sm text-muted-foreground" data-testid="patronus-start-no-push-note">
          {`The remote branch of ${branchName} has the same commits, only on another baseline: nothing is pushed, Patronus starts from it.`}
        </p>
      )}
      {startPush.kind === "unknown" && (
        <p className="flex items-center gap-2 text-sm text-muted-foreground" data-testid="patronus-start-remote-loading">
          <Spinner className="size-4" />
          {`Comparing ${branchName} with its remote branch…`}
        </p>
      )}
      {dismissesApprovals && fromPullRequest !== null && (
        <p className="flex items-start gap-1.5 text-sm text-warning" data-testid="patronus-start-dismisses-approvals">
          <TriangleAlertIcon className="mt-0.5 size-4 shrink-0" />
          {`The push dismisses the approvals of #${fromPullRequest.number}: the rules of the repository dismiss stale reviews when new commits are pushed.`}
        </p>
      )}
      {blocked && (
        <p className="flex items-start gap-1.5 text-sm text-warning" data-testid="patronus-start-blocked">
          <TriangleAlertIcon className="mt-0.5 size-4 shrink-0" />
          {`A ${formatPushMode(robot!.pushMode)} of this commit is running.`}
        </p>
      )}
    </InlineForm>
  )
}

/**
 * The lead of a stacked branch's Safe Push: the unstack, the move of the commits it needs, or the Safe Push of the root
 * first. When the first action is another one, the Safe Push of the root follows as a link. While the trial build of
 * the branch alone runs, a muted line says so.
 */
function StackLeadBlock({
  lead,
  checking,
  branchName,
  disabled,
  onUnstack,
  onMove,
}: {
  lead: StackLead
  checking: boolean
  branchName: string
  disabled: boolean
  onUnstack: () => void
  onMove: (commits: readonly StackCommit[]) => void
}) {
  const { action, root } = lead
  const safePushRoot = () => openInline("safe-push", root)
  return (
    <div className="flex flex-col gap-2 text-sm" data-testid="patronus-start-stack-lead" data-action={action.kind}>
      <p>{lead.text}</p>
      {checking && (
        <p className="flex items-center gap-2 text-muted-foreground" data-testid="patronus-start-stack-checking">
          <Spinner className="size-4" />
          {`Checking whether ${branchName} builds alone…`}
        </p>
      )}
      <div className="flex flex-wrap items-center gap-2">
        {action.kind === "safePushRoot" && (
          <Button variant="outline" size="xs" onClick={safePushRoot}>
            <ShieldCheckIcon data-icon="inline-start" />
            {`Safe Push ${root}…`}
          </Button>
        )}
        {action.kind === "safePushRoot" && action.unstack !== undefined && (
          <Button variant="link" size="xs" className="h-auto px-0" disabled={disabled} onClick={onUnstack}>
            {`Unstack from ${action.unstack}`}
          </Button>
        )}
        {action.kind === "unstack" && (
          <Button variant="outline" size="xs" disabled={disabled} onClick={onUnstack}>
            <Layers2Icon data-icon="inline-start" />
            {`Unstack from ${action.parent}`}
          </Button>
        )}
        {action.kind === "move" && (
          <Button variant="outline" size="xs" disabled={disabled} onClick={() => onMove(action.commits)}>
            <ArrowDownToLineIcon data-icon="inline-start" />
            {action.label}
          </Button>
        )}
        {action.kind !== "safePushRoot" && (
          <Button variant="link" size="xs" className="h-auto px-0" onClick={safePushRoot}>
            {`Safe Push ${root}…`}
          </Button>
        )}
      </div>
    </div>
  )
}
