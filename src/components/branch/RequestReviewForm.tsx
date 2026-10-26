import { useState } from "react"
import { TriangleAlertIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { PopoverDescription, PopoverSectionTitle, PopoverTitle } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { ReviewerPicker } from "@/components/shared/ReviewerPicker"
import { useReviewerSearch } from "@/hooks/useReviewerSearch"
import type { GitHubUser, PullRequest, ReviewReadiness } from "@/lib/bindings"
import { isRulesUnknown } from "@/lib/branch-rules"
import { isLogin } from "@/lib/create-pull-request"
import { notifyError } from "@/lib/errors"
import { openPullRequest } from "@/lib/pull-request"
import { pullRequestOperationIn, requestReview } from "@/lib/pull-request-run"
import { showToast } from "@/lib/toast-message"
import { useRowPart } from "@/model/store"
import { actionBlocker } from "@/presenters/actions"
import { describeCodeOwnersFact, formatCandidate, formatRefused, formatRequestedToast } from "@/presenters/review"
import type { Branch } from "@/stores/branch-sync"
import { useBranchSync, useOpenPullRequest } from "@/stores/branch-sync"
import { usePatronusAvailable } from "@/stores/patronus"
import { useBranchRules } from "@/stores/pull-requests"
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
 * The body of `RequestReviewPopover`, mounted on each opening: the choice starts again from the suggested code owners,
 * also those of a readiness that comes after the opening, until the user checks or unchecks one
 */
export function RequestReviewForm({ branch, pullRequest, readiness, onDone }: RequestReviewFormProps) {
  const repositoryPath = useSelectedProjectPath() ?? ""
  // the open pull request as Rust reads it: a draft, the code owners Request Review… offers (`review::candidates`)
  const view = useOpenPullRequest(branch.name)
  const candidates = view?.candidates ?? []
  // null until the user changes the choice: the suggested ones of the readiness as it is now
  const [choice, setChoice] = useState<readonly string[] | null>(null)
  const checked = choice ?? candidates.filter(candidate => candidate.preselected).map(candidate => candidate.login)
  const [others, setOthers] = useState<GitHubUser[]>([])
  const search = useReviewerSearch(repositoryPath)
  // the request is an operation of the session (a second click joins it): its spinner while it runs, from any window
  const isRequesting = useBranchSync(state => pullRequestOperationIn(state.operations, "requestReview", branch.name) !== undefined)

  // as Rust offers the request (a draft asks nobody)
  const actions = useRowPart(branch.name, "actions")
  const blocker = actionBlocker(actions, "request-review", { subject: branch.name }) ?? null
  // the reviewers see the commits: the request pushes the work the remote lacks first (a step of its plan, not of the
  // user); the warnings read Rust's push action of the remote status (an unknown one is no push)
  const pushes = branch.remoteStatus != null && branch.remoteStatus.action.kind !== "upToDate"
  // a push of new commits into a pull request someone approved: the rules of the repository may dismiss the approvals
  const approved = pushes && pullRequest.reviewers.some(reviewer => reviewer.state === "approved")
  // only a warning here: the request counts no approvals, so it never waits for the rules; rules it can't read are said
  // as such, never as "keeps them"
  const rules = useBranchRules(pullRequest.baseRefName || null, approved)
  const dismissesApprovals = rules === "dismiss"
  const rulesUnknown = rules !== null && isRulesUnknown(rules) ? rules.reason : null
  // whom Patronus asks for: a check that failed or doesn't tell yet is said, never "no code owner needed"
  const codeOwners = describeCodeOwnersFact(readiness, usePatronusAvailable())
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
    // the repository of the click: the push and the request go there, whichever the main window shows meanwhile
    void requestReview(branch.name, logins, repositoryPath).then(
      result => {
        const toast = formatRequestedToast(pullRequest, result, nameOf)
        if (toast === null) {
          notifyError(errorTitle, new Error(formatRefused(result, nameOf)))
          return
        }
        showToast({ ...toast, action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } })
        onDone()
      },
      (error: unknown) => notifyError(errorTitle, error),
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
      {blocker === null && codeOwners !== null && (
        <p className="text-xs text-muted-foreground" data-testid="request-review-code-owners-unknown">
          {codeOwners}
        </p>
      )}
      {blocker === null && rulesUnknown !== null && (
        <p className="flex items-start gap-1.5 text-xs text-warning" data-testid="request-review-rules-unknown">
          <TriangleAlertIcon className="mt-0.5 size-3.5 shrink-0" />
          {`Cannot check whether the push of the new commits keeps the approvals of #${number}: ${rulesUnknown}.`}
        </p>
      )}
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
