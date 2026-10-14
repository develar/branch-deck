import { useEffect, useState } from "react"
import { FlaskConicalIcon, ShieldCheckIcon, TriangleAlertIcon } from "lucide-react"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { PatronusReviewBlock } from "@/components/branch/PatronusReviewBlock"
import { InlineForm } from "@/components/shared/InlineForm"
import { usePatronusReviewRequirement } from "@/hooks/usePatronusReviewRequirement"
import type { PatronusPushMode, PatronusStartOptions } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { pushBranch } from "@/lib/git/push"
import { allowsPushMode, formatPushMode, isStartBlocked } from "@/lib/patronus"
import { branchSyncStore, useBranchRemoteTip, useBranchTip } from "@/stores/branch-sync"
import { closeInline, useIsRowProcessing, withRowProcessing } from "@/stores/inline-row-action"
import { getBranchRobot, patronusRequest, startPatronusRobot, useBranchRobot } from "@/stores/patronus"

/** The Select value of "no profile": Patronus runs the default checks of the target branch */
const NO_PROFILE = "\0none"

type StartOptionsState = { status: "loading" } | { status: "loaded", options: PatronusStartOptions } | { status: "error", message: string }

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
 * robot by default), what will happen, and Start. A branch whose tip the remote lacks is pushed first.
 */
export function InlinePatronusStart({ branchName, mode }: { branchName: string, mode: Extract<PatronusPushMode, "rebaseAutosquash" | "dryRun"> }) {
  const state = useStartOptions(branchName)
  const processing = useIsRowProcessing(branchName)
  const localTip = useBranchTip(branchName)
  const remoteTip = useBranchRemoteTip(branchName)
  const robot = useBranchRobot(branchName)
  const [chosenProfile, setChosenProfile] = useState<string | null>(null)
  const [isPushing, setPushing] = useState(false)

  const modeName = formatPushMode(mode)
  const options = state.status === "loaded" ? state.options : null
  const profiles = options?.profiles.filter(profile => allowsPushMode(profile, mode)) ?? []
  const defaultProfile = profiles.some(profile => profile.id === options?.defaultProfileId) ? options!.defaultProfileId! : NO_PROFILE
  const profile = chosenProfile ?? defaultProfile
  const needsPush = remoteTip !== localTip
  const blocked = isStartBlocked(robot, localTip)
  const canStart = options !== null && options.supported && !blocked && !processing && !isPushing && localTip !== undefined
  // the code owners don't check a dry run
  const review = usePatronusReviewRequirement(branchName, mode === "rebaseAutosquash" && options?.supported === true)
  const mergeRequest = review.state.status === "loaded" ? review.state.requirement.mergeRequest : null

  /**
   * The branch on the remote with its tip: a start and a merge request need it. False when the push failed (it said so).
   * A Safe Push running on an older commit is canceled for the push, not restarted: the form starts the new one, which
   * has the merge request. A dry run doesn't replace it, so the push of a dry run restarts it.
   */
  const pushIfNeeded = async (): Promise<boolean> => {
    if (!needsPush) {
      return true
    }
    setPushing(true)
    const pushed = await pushBranch(branchName, { startsRobot: mode === "rebaseAutosquash" })
    setPushing(false)
    return pushed
  }

  const start = async () => {
    if (!(await pushIfNeeded())) {
      return
    }
    await withRowProcessing(branchName, () => startPatronusRobot(branchName, mode, profile === NO_PROFILE ? null : profile, mergeRequest), {
      processingMessage: `Starting the ${modeName}…`,
      success: started => ({ title: `${modeName} started`, description: started.name }),
      error: error => ({ title: `Cannot start the ${modeName}`, description: error instanceof Error ? error.message : String(error) }),
    })
  }

  const target = options?.targetBranch ?? "the baseline"
  const explanation = mode === "dryRun"
    ? `Patronus rebases ${branchName} onto ${target} and runs the checks, without pushing into ${target}.`
    : `Patronus rebases ${branchName} onto ${target}, runs the checks and pushes it into ${target} when they pass.`
  // a dry run doesn't push into the target, but Patronus starts from the remote branch: the local tip goes there first
  const unpushed = remoteTip === null ? branchName : `The last commit of ${branchName}`
  const pushNote = `${unpushed} isn't on the remote yet: it is pushed first, then Patronus starts from the remote branch.`
  const items = [
    { value: NO_PROFILE, label: "No profile" },
    ...profiles.map(it => ({ value: it.id, label: it.name })),
  ]
  return (
    <InlineForm
      title={`${modeName} ${branchName}`}
      description={explanation}
      dataTestId="patronus-start-form"
      canSubmit={canStart}
      loading={processing || isPushing}
      submitText={needsPush ? `Push and Start ${modeName}` : `Start ${modeName}`}
      submitIcon={mode === "dryRun" ? <FlaskConicalIcon data-icon="inline-start" /> : <ShieldCheckIcon data-icon="inline-start" />}
      onCancel={closeInline}
      onSubmit={() => void start()}
    >
      <p className="text-sm" data-testid="patronus-start-explanation">{explanation}</p>
      {state.status === "loading" && (
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <Spinner className="size-3.5" />
          Asking Patronus for the merge profiles…
        </p>
      )}
      {state.status === "error" && <p className="text-sm text-destructive" data-testid="patronus-start-error">{state.message}</p>}
      {options !== null && !options.supported && (
        <p className="text-sm text-destructive" data-testid="patronus-start-error">{`Patronus does not push into ${options.targetBranch}.`}</p>
      )}
      {options !== null && options.supported && (
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
          review={review}
          onCreateMergeRequest={async () => (await pushIfNeeded()) ? (await review.createMergeRequest()).url : null}
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
