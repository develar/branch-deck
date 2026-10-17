import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { PopoverDescription, PopoverSectionTitle, PopoverTitle } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { ReviewerPicker } from "@/components/shared/ReviewerPicker"
import { useReviewerSearch } from "@/hooks/useReviewerSearch"
import type { GitHubUser, PullRequest, ReviewReadiness } from "@/lib/bindings"
import { isLogin } from "@/lib/create-pull-request"
import { notifyError } from "@/lib/errors"
import { openPullRequest } from "@/lib/pull-request"
import { formatCandidate, formatRefused, formatRequestedToast, getRequestReviewBlocker, getRequestReviewCandidates } from "@/lib/request-review"
import { showToast } from "@/lib/toast-message"
import type { Branch } from "@/stores/branch-sync"
import { requestReviewers } from "@/stores/pull-requests"
import { useSelectedProjectPath } from "@/stores/repository"

export interface RequestReviewFormProps {
  branch: Branch
  pullRequest: PullRequest
  readiness: ReviewReadiness | null
  /** Closes the popover after a request GitHub asked someone in */
  onDone: () => void
}

/** The body of `RequestReviewPopover`, mounted on each opening: the choice starts again from the suggested code owners */
export function RequestReviewForm({ branch, pullRequest, readiness, onDone }: RequestReviewFormProps) {
  const repositoryPath = useSelectedProjectPath() ?? ""
  const candidates = getRequestReviewCandidates(readiness, pullRequest)
  const [checked, setChecked] = useState<readonly string[]>(() => candidates.filter(candidate => candidate.preselected).map(candidate => candidate.login))
  const [others, setOthers] = useState<GitHubUser[]>([])
  const search = useReviewerSearch(repositoryPath)
  const [isRequesting, setIsRequesting] = useState(false)

  const blocker = getRequestReviewBlocker(branch, pullRequest)
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
    setChecked(on ? [...checked, login] : checked.filter(it => !isLogin(it, login)))
  }

  const request = async () => {
    if (!canRequest) {
      return
    }
    setIsRequesting(true)
    const errorTitle = `Cannot request a review on #${number}`
    // `.finally`, not try/finally: React Compiler cannot compile a try without a catch
    const result = await requestReviewers(branch.name, logins)
      .catch((error: unknown) => {
        notifyError(errorTitle, error)
        return null
      })
      .finally(() => setIsRequesting(false))
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

      <div className="flex justify-end">
        <Button size="xs" data-testid="request-review-submit" disabled={!canRequest} onClick={() => void request()}>
          {isRequesting && <Spinner data-icon="inline-start" />}
          Request Review
        </Button>
      </div>
    </>
  )
}
