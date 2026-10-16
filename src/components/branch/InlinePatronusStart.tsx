import { useEffect, useState } from "react"
import { FlaskConicalIcon, ShieldCheckIcon, TriangleAlertIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { PatronusReviewBlock } from "@/components/branch/PatronusReviewBlock"
import { InlineForm } from "@/components/shared/InlineForm"
import { usePatronusReviewRequirement } from "@/hooks/usePatronusReviewRequirement"
import type { PatronusPushMode, PatronusStartOptions } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { getNewPullRequestUrl, openNewPullRequest } from "@/lib/git/new-pull-request"
import { pushBranch, pushStack } from "@/lib/git/push"
import { allowsPushMode, formatPushMode, getPullRequestStart, isStartBlocked } from "@/lib/patronus"
import { describeStackStart } from "@/lib/patronus-stack"
import { announceRobot } from "@/lib/patronus-actions"
import { stackAncestors, stackPushOrder } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import { branchSyncStore, getBranch, stackDescendantsIn, useBranchRemoteTip, useBranchSync, useBranchTip } from "@/stores/branch-sync"
import { closeInline, openInline, useIsRowProcessing, withRowProgress } from "@/stores/inline-row-action"
import { getBranchRobot, patronusRequest, startPatronusFromPullRequest, startPatronusRobot, useBranchRobot, useLandingAncestor, useLandingSpan, usePatronusGitHubRepository } from "@/stores/patronus"
import { getPullRequest, usePullRequest } from "@/stores/pull-requests"
import { pinStackPath } from "@/stores/stack-view"

/** The Select value of "no profile": Patronus runs the default checks of the target branch */
const NO_PROFILE = "\0none"

type StartOptionsState = { status: "loading" } | { status: "loaded", options: PatronusStartOptions } | { status: "error", message: string }

/** Names, one per line, as a selector returns them so the store notifies only when they change */
function splitNames(names: string): string[] {
  return names === "" ? [] : names.split("\n")
}

/** The target branch and the merge profiles, asked once when the form opens (Patronus caches nothing for the app) */
function useStartOptions(branchName: string): StartOptionsState {
  const [state, setState] = useState<StartOptionsState>({ status: "loading" })
  useEffect(() => {
    let current = true
    commands.getPatronusStartOptions({
      repositoryPath: patronusRequest().repositoryPath,
      baselineBranch: branchSyncStore.getState().baselineBranch ?? "",
      lastRobotId: getBranchRobot(branchName)?.id ?? null,
    }).then((result) => {
      if (current) {
        setState(result.status === "ok" ? { status: "loaded", options: result.data } : { status: "error", message: result.error.message })
      }
    }, (error: unknown) => {
      if (current) {
        setState({ status: "error", message: error instanceof Error ? error.message : String(error) })
      }
    })
    return () => {
      current = false
    }
  }, [branchName])
  return state
}

/**
 * Starts a Safe Push (`rebaseAutosquash`) or a Dry Run of the branch: the merge profile (the one of the branch's last
 * robot by default), what will happen, and Start. A branch whose tip the remote lacks is pushed first. A branch with an
 * open pull request in the GitHub repository the branches are mirrored to starts from it, a `/patronus` comment: then
 * the approvals of the pull request count and Patronus closes it once merged; a comment can't choose a profile.
 *
 * A stacked branch carries the commits of the branches it is built on: its Safe Push lands them too. The form leads root
 * first (a Safe Push of the root-most ancestor, whose merge moves the branch onto the target), and Start lands the stack
 * in one robot: the ancestors with work for the remote are pushed first, and a pull request into the parent's branch
 * isn't the start (Patronus merges a pull request into its base), the API is. A Safe Push of a parent says what its
 * merge does to the branches stacked on it.
 */
export function InlinePatronusStart({ branchName, mode }: { branchName: string, mode: Extract<PatronusPushMode, "rebaseAutosquash" | "dryRun"> }) {
  const state = useStartOptions(branchName)
  const processing = useIsRowProcessing(branchName)
  const localTip = useBranchTip(branchName)
  const remoteTip = useBranchRemoteTip(branchName)
  const robot = useBranchRobot(branchName)
  const githubRepository = usePatronusGitHubRepository()
  const pullRequest = usePullRequest(branchName)
  const [chosenProfile, setChosenProfile] = useState<string | null>(null)
  const [isPushing, setPushing] = useState(false)
  const ancestors = splitNames(useBranchSync(state => stackAncestors(branchName, name => state.branches[name]).map(it => it.name).join("\n")))
  const children = splitNames(useBranchSync(state => stackDescendantsIn(state, branchName).join("\n")))
  const ancestorCommitCount = useBranchSync(state => ancestors.reduce((sum, name) => sum + (state.branches[name]?.commitCount ?? 0), 0))
  // the branches the start pushes, root first: the stack while an ancestor has work for the remote
  const pushOrder = splitNames(useBranchSync(state => stackPushOrder(branchName, name => state.branches[name], { stopAtConfirm: true }).join("\n")))
  // the stack graph shows what the start lands while the form is open
  useEffect(() => {
    pinStackPath(branchName, true)
    return () => pinStackPath(branchName, false)
  }, [branchName])
  const landingAncestor = useLandingAncestor(branchName)
  const landingSpan = useLandingSpan(branchName)

  const modeName = formatPushMode(mode)
  const options = state.status === "loaded" ? state.options : null
  const fromPullRequest = getPullRequestStart(githubRepository, pullRequest, mode, options?.targetBranch ?? null)
  const profiles = options?.profiles.filter(profile => allowsPushMode(profile, mode)) ?? []
  const defaultProfile = profiles.some(profile => profile.id === options?.defaultProfileId) ? options!.defaultProfileId! : NO_PROFILE
  const profile = chosenProfile ?? defaultProfile
  const pushesStack = pushOrder.length > 1
  const needsPush = remoteTip !== localTip || pushesStack
  const blocked = isStartBlocked(robot, localTip)
  // a dry run pushes nothing into the target: it may run while an ancestor lands, or while a descendant lands the branch
  const waitsForAncestor = mode === "rebaseAutosquash" ? landingAncestor : null
  const landsWithDescendant = mode === "rebaseAutosquash" ? landingSpan : null
  const canStart = options !== null && options.supported && !blocked && waitsForAncestor === null && landsWithDescendant === null && !processing && !isPushing && localTip !== undefined && fromPullRequest?.blockedByDraft !== true
  // the code owners don't check a dry run
  const review = usePatronusReviewRequirement(branchName, mode === "rebaseAutosquash" && options?.supported === true)
  const mergeRequest = review.state.status === "loaded" ? review.state.requirement.mergeRequest : null

  /**
   * The branch on the remote with its tip: a start and a merge request need it. False when the push failed (it said so).
   * A Safe Push running on an older commit is canceled for the push, not restarted: the form starts the new one, which
   * has the merge request. A dry run doesn't replace it, so the push of a dry run restarts it. The push is a step of
   * the form's action: the start's toast, the opened merge request or pull request say how it went, not a push toast.
   */
  const pushIfNeeded = async (): Promise<boolean> => {
    if (!needsPush) {
      return true
    }
    setPushing(true)
    const startsRobot = mode === "rebaseAutosquash"
    const pushed = pushesStack
      ? await pushStack(branchName, { stopAtConfirm: true, startsRobot, announce: false })
      : await pushBranch(branchName, { startsRobot, announce: false })
    setPushing(false)
    return pushed
  }

  const start = async () => {
    if (!(await pushIfNeeded())) {
      return
    }
    if (fromPullRequest !== null) {
      await withRowProgress(branchName, async () => {
        try {
          announceRobot(`${modeName} started`, await startPatronusFromPullRequest(fromPullRequest.number, mode))
          return true
        }
        catch (error) {
          notifyError(`Cannot start the ${modeName}`, error)
          return false
        }
      }, { processingMessage: `Asking Patronus on pull request #${fromPullRequest.number}…` })
      return
    }
    const profileId = profile === NO_PROFILE ? null : profile
    const profileName = profiles.find(it => it.id === profileId)?.name ?? null
    await withRowProgress(branchName, async () => {
      try {
        announceRobot(`${modeName} started`, await startPatronusRobot(branchName, mode, profileId, mergeRequest, profileName), profileName)
        return true
      }
      catch (error) {
        notifyError(`Cannot start the ${modeName}`, error)
        return false
      }
    }, { processingMessage: `Starting the ${modeName}…` })
  }

  /** The branch on the remote with its tip, then the GitHub form of its pull request, where the approvals count */
  const createPullRequest = async () => {
    if (!(await pushIfNeeded())) {
      return
    }
    const branch = getBranch(branchName)
    const url = branch === undefined ? null : getNewPullRequestUrl(branch)
    if (url !== null) {
      openNewPullRequest(url)
    }
  }

  const target = options?.targetBranch ?? "the baseline"
  const robotWork = mode === "dryRun"
    ? `Patronus rebases ${branchName} onto ${target} and runs the checks, without pushing into ${target}.`
    : `Patronus rebases ${branchName} onto ${target}, runs the checks and pushes it into ${target} when they pass.`
  const explanation = fromPullRequest === null
    ? robotWork
    : `Comments ${fromPullRequest.command} on pull request #${fromPullRequest.number}: ${robotWork} It reports in the pull request${mode === "dryRun" ? "" : " and closes it once merged"}.`
  // a dry run doesn't push into the target, but Patronus starts from the remote branch: the local tip goes there first
  const unpushed = remoteTip === null ? branchName : `The last commit of ${branchName}`
  const pushNote = pushesStack
    ? `${joinNames(pushOrder)} have commits the remote lacks: they are pushed first, parents first, then Patronus starts from the remote branch of ${branchName}.`
    : `${unpushed} isn't on the remote yet: it is pushed first, then Patronus starts from the remote branch.`
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
  })
  const items = [
    { value: NO_PROFILE, label: "No profile" },
    ...profiles.map(it => ({ value: it.id, label: it.name })),
  ]
  return (
    <InlineForm
      title={`${modeName} of ${branchName}`}
      description={<span data-testid="patronus-start-explanation">{explanation}</span>}
      dataTestId="patronus-start-form"
      canSubmit={canStart}
      loading={processing || isPushing}
      submitText={needsPush ? `${pushesStack ? "Push Stack" : "Push"} and Start ${modeName}` : `Start ${modeName}`}
      submitIcon={mode === "dryRun" ? <FlaskConicalIcon data-icon="inline-start" /> : <ShieldCheckIcon data-icon="inline-start" />}
      // a Safe Push of a stacked branch lands its ancestors too: Enter doesn't choose it, the lead is root first
      focusSubmit={stackNotes.lead === null}
      onCancel={closeInline}
      onSubmit={() => void start()}
    >
      {stackNotes.lead !== null && (
        <div className="flex flex-col gap-2 text-sm" data-testid="patronus-start-stack-lead">
          <p>{stackNotes.lead.text}</p>
          <Button variant="outline" size="xs" className="self-start" onClick={() => openInline("safe-push", stackNotes.lead!.root)}>
            <ShieldCheckIcon data-icon="inline-start" />
            {`Safe Push ${stackNotes.lead.root}…`}
          </Button>
        </div>
      )}
      {stackNotes.note !== null && (
        <p className="text-sm text-muted-foreground" data-testid="patronus-start-stack-note">{stackNotes.note}</p>
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
      {state.status === "loading" && (
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <Spinner className="size-4" />
          Asking Patronus for the merge profiles…
        </p>
      )}
      {state.status === "error" && <p className="text-sm text-destructive" data-testid="patronus-start-error">{state.message}</p>}
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
              {items.map(item => <SelectItem key={item.value} value={item.value}>{item.label}</SelectItem>)}
            </SelectContent>
          </Select>
        </label>
      )}
      {mode === "rebaseAutosquash" && options?.supported === true && (
        <PatronusReviewBlock
          branchName={branchName}
          review={review}
          github={githubRepository === null ? null : { pullRequest }}
          onCreatePullRequest={createPullRequest}
        />
      )}
      {needsPush && (
        <p className="text-sm text-muted-foreground" data-testid="patronus-start-push-note">{pushNote}</p>
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
