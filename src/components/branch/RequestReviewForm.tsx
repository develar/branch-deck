import { useState } from "react"
import { TriangleAlertIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { PopoverDescription, PopoverSectionTitle, PopoverTitle } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { ReviewerPicker } from "@/components/shared/ReviewerPicker"
import { useDismissesStaleReviews } from "@/hooks/useDismissesStaleReviews"
import { useReviewerSearch } from "@/hooks/useReviewerSearch"
import type { GitHubUser, PullRequest, ReviewReadiness } from "@/lib/bindings"
import { isLogin } from "@/lib/create-pull-request"
import { notifyError } from "@/lib/errors"
import { pushForAction } from "@/lib/git/push"
import { pushWholeStack } from "@/lib/git/stack-push"
import { openPullRequest } from "@/lib/pull-request"
import { formatCandidate, formatRefused, formatRequestedToast, getRequestReviewBlocker, getRequestReviewCandidates, requestReviewPushes } from "@/lib/request-review"
import { showToast } from "@/lib/toast-message"
import { stackRootOfReadinessKey } from "@/lib/whole-stack"
import type { Branch } from "@/stores/branch-sync"
import { runOperation, useIndicatorClaim, useSurfaceBusy } from "@/stores/operations"
import { requestReviewers } from "@/stores/pull-requests"
import { useSelectedProjectPath } from "@/stores/repository"

export interface RequestReviewFormProps {
  /**
   * The branch of the pull request, or the whole stack of a root: its readiness key (`stackReadinessKey`) and the remote
   * status of its stack branch
   */
  branch: Pick<Branch, "name" | "remoteStatus">
  pullRequest: PullRequest
  readiness: ReviewReadiness | null
  /** Closes the popover after a request GitHub asked someone in */
  onDone: () => void
}

/**
 * The push of the work the remote lacks before the request, reported under `errorTitle`: of the branch (`pushForAction`,
 * its stack first), or of the stack branch of the whole stack (`pushWholeStack`, only a built one)
 */
async function pushFirst(name: string, errorTitle: string): Promise<boolean> {
  const root = stackRootOfReadinessKey(name)
  if (root !== null) {
    return pushWholeStack(root, { report: (_title, reason) => notifyError(errorTitle, reason) })
  }
  const pushed = await pushForAction(name)
  if (!pushed.pushed) {
    notifyError(errorTitle, pushed.reason)
  }
  return pushed.pushed
}

/**
 * The body of `RequestReviewPopover`, mounted on each opening: the choice starts again from the suggested code owners,
 * also those of a readiness that comes after the opening, until the user checks or unchecks one
 */
export function RequestReviewForm({ branch, pullRequest, readiness, onDone }: RequestReviewFormProps) {
  const repositoryPath = useSelectedProjectPath() ?? ""
  const candidates = getRequestReviewCandidates(readiness, pullRequest)
  // null until the user changes the choice: the suggested ones of the readiness as it is now
  const [choice, setChoice] = useState<readonly string[] | null>(null)
  const checked = choice ?? candidates.filter(candidate => candidate.preselected).map(candidate => candidate.login)
  const [others, setOthers] = useState<GitHubUser[]>([])
  const search = useReviewerSearch(repositoryPath)
  // the request runs from here: one operation of the pull request (a second click joins it), its spinner here
  const surface = `request-review:${branch.name}`
  useIndicatorClaim(surface)
  const isRequesting = useSurfaceBusy(surface) !== null

  const blocker = getRequestReviewBlocker(pullRequest)
  // the reviewers see the commits: the request pushes the work the remote lacks first, a step of it, not of the user
  const pushes = requestReviewPushes(branch)
  // a push of new commits into a pull request someone approved: the rules of the repository may dismiss the approvals
  const approved = pushes && pullRequest.reviewers.some(reviewer => reviewer.state === "approved")
  // only a warning here: the request counts no approvals, so it never waits for the rules
  const dismissesApprovals = useDismissesStaleReviews(pullRequest.baseRefName || null, approved) === true
  const logins = candidates.filter(candidate => checked.some(login => isLogin(login, candidate.login))).map(candidate => candidate.login)
  for (const user of others) {
    if (!logins.some(login => isLogin(login, user.login))) {
      logins.push(user.login)
    }
  }
  const canRequest = !isRequesting && blocker === null && logins.length > 0
  const number = pullRequest.number

  /** The full name the popover knows, else the login */
  const nameOf = (login: string) => {
    const candidate = candidates.find(it => isLogin(it.login, login))
    return candidate === undefined ? (others.find(user => isLogin(user.login, login))?.name ?? login) : formatCandidate(candidate)
  }

  const toggle = (login: string, on: boolean) => {
    setChoice(on ? [...checked, login] : checked.filter(it => !isLogin(it, login)))
  }

  const request = () => {
    if (!canRequest) {
      return
    }
    const errorTitle = `Cannot request a review on #${number}`
    const run = async () => {
      if (pushes && !(await pushFirst(branch.name, errorTitle))) {
        return
      }
      const result = await requestReviewers(branch.name, logins).catch((error: unknown) => {
        notifyError(errorTitle, error)
        return null
      })
      if (result === null) {
        return
      }
      const toast = formatRequestedToast(pullRequest, result, nameOf)
      if (toast === null) {
        notifyError(errorTitle, new Error(formatRefused(result, nameOf)))
        return
      }
      showToast({ ...toast, action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } })
      onDone()
    }
    void runOperation({ id: "request-review", subject: `pr:${number}`, objects: [branch.name], kind: "remote", label: `Requesting a review on #${number}`, surface }, run, reason =>
      showToast({ type: "info", title: errorTitle, description: reason }),
    )
  }

  return (
    <>
      <div className="flex flex-col gap-1">
        <PopoverTitle className="text-sm">{`Request a review on #${number}`}</PopoverTitle>
        <PopoverDescription className="truncate text-xs">{pullRequest.title}</PopoverDescription>
      </div>

      {candidates.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <PopoverSectionTitle>Code owners</PopoverSectionTitle>
          <ul className="flex flex-col gap-0.5" data-testid="request-review-candidates">
            {candidates.map(candidate => (
              <li key={candidate.login}>
                <label className="flex min-w-0 items-start gap-2 rounded-lg px-1 py-1 hover:bg-muted">
                  <Checkbox
                    className="mt-0.5"
                    data-testid={`request-review-candidate-${candidate.login}`}
                    checked={checked.some(login => isLogin(login, candidate.login))}
                    disabled={isRequesting}
                    onCheckedChange={on => toggle(candidate.login, on)}
                  />
                  <span className="flex min-w-0 flex-col">
                    <span className="flex min-w-0 items-baseline gap-1.5">
                      <span className="truncate">{formatCandidate(candidate)}</span>
                      {candidate.fullName !== null && <span className="shrink-0 text-xs text-muted-foreground">{candidate.login}</span>}
                    </span>
                    <span className="truncate text-xs text-muted-foreground">{candidate.groups.join(", ")}</span>
                  </span>
                </label>
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="flex flex-col gap-1.5">
        <PopoverSectionTitle>{candidates.length > 0 ? "Others" : "Reviewers"}</PopoverSectionTitle>
        <ReviewerPicker testIdPrefix="request-review" reviewers={others} setReviewers={setOthers} search={search} viewerLogin={pullRequest.author} disabled={isRequesting} />
      </div>

      {blocker !== null && <p className="text-xs text-muted-foreground">{blocker}</p>}
      {blocker === null && dismissesApprovals && (
        <p className="flex items-start gap-1.5 text-xs text-warning" data-testid="request-review-dismisses-approvals">
          <TriangleAlertIcon className="mt-0.5 size-3.5 shrink-0" />
          {`The push of the new commits dismisses the approvals of #${number}: the rules of the repository dismiss stale reviews when new commits are pushed.`}
        </p>
      )}

      <div className="flex justify-end">
        <Button size="xs" data-testid="request-review-submit" disabled={!canRequest} onClick={request}>
          {isRequesting && <Spinner data-icon="inline-start" />}
          Request Review
        </Button>
      </div>
    </>
  )
}
