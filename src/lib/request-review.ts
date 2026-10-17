import { UserPlusIcon } from "lucide-react"
import type { MissingReviewGroup, Person, PullRequest, RequestedReviewers, ReviewReadiness } from "@/lib/bindings"
import { isLogin, preselectedReviewers } from "@/lib/create-pull-request"
import type { MenuItem } from "@/lib/menu"
import { getPushAction } from "@/lib/push-summary"
import { joinNames } from "@/lib/text"
import type { ToastMessage } from "@/lib/toast-message"
import type { Branch } from "@/stores/branch-sync"

/** A code owner Request Review… offers: suggested by Patronus for a group whose approval is missing */
export interface RequestReviewCandidate {
  readonly login: string
  /** The full name Patronus knows; null when it names none */
  readonly fullName: string | null
  /** The missing groups that suggest them, in the order of the readiness */
  readonly groups: readonly string[]
  /**
   * Chosen when the popover opens: one suggested code owner of each missing group nobody is asked for yet, each once,
   * as the Create Pull Request dialog chooses (`preselectedReviewers`); `bd request-review` asks the same
   */
  readonly preselected: boolean
}

/**
 * The suggested code owners with a GitHub login of the groups whose approval is missing (`readiness` `missing`), once
 * each; never the author (GitHub refuses a review request of the author), a member asked already, or one whose
 * approval counts. None for any other readiness: the search finds whom else to ask.
 */
export function getRequestReviewCandidates(readiness: ReviewReadiness | null, pullRequest: PullRequest): RequestReviewCandidate[] {
  if (readiness?.kind !== "missing") {
    return []
  }
  const asked = (login: string) => pullRequest.reviewers.some(reviewer => reviewer.requested && isLogin(reviewer.login, login))
  const isEligible = (group: MissingReviewGroup, person: Person): person is Person & { login: string } =>
    person.login !== null &&
    !isLogin(person.login, pullRequest.author) &&
    !asked(person.login) &&
    !group.requested.some(it => isLogin(it.login, person.login)) &&
    !group.approvedBy.some(it => isLogin(it.login, person.login))

  const suggestions = readiness.groups.flatMap(group =>
    group.suggested
      .filter(person => isEligible(group, person))
      .map(person => ({ group: group.name, login: person.login, fullName: person.fullName, waiting: group.requested.length > 0 })),
  )
  const preselected = preselectedReviewers(
    suggestions.filter(suggestion => !suggestion.waiting).map(({ group, login, fullName }) => ({ group, login, fullName: fullName ?? login })),
  )

  const candidates: { login: string; fullName: string | null; groups: string[]; preselected: boolean }[] = []
  for (const suggestion of suggestions) {
    const known = candidates.find(candidate => isLogin(candidate.login, suggestion.login))
    if (known === undefined) {
      candidates.push({
        login: suggestion.login,
        fullName: suggestion.fullName,
        groups: [suggestion.group],
        preselected: preselected.some(user => isLogin(user.login, suggestion.login)),
      })
    } else if (!known.groups.includes(suggestion.group)) {
      known.groups.push(suggestion.group)
    }
  }
  return candidates
}

/** "Ilya Korennoy", else the login */
export function formatCandidate(candidate: Pick<RequestReviewCandidate, "login" | "fullName">): string {
  return candidate.fullName ?? candidate.login
}

/**
 * "Request Review…" of a branch, which opens the popover of its pull request (`open`): disabled with the reason while
 * the pull request is a draft or the branch has work the remote lacks (the reviewers would review old commits); null
 * without an open pull request, where it never applies (Create Pull Request… asks the reviewers then)
 */
export function getRequestReviewItem(
  branch: Pick<Branch, "remoteStatus">,
  pullRequest: PullRequest | null | undefined,
  readiness: ReviewReadiness | null,
  open: () => void,
): MenuItem | null {
  if (pullRequest == null || pullRequest.state !== "open") {
    return null
  }
  const reason = getRequestReviewBlocker(branch, pullRequest)
  const chosen = getRequestReviewCandidates(readiness, pullRequest).filter(candidate => candidate.preselected)
  return {
    id: "request-review",
    label: "Request Review…",
    icon: UserPlusIcon,
    disabled: reason !== null,
    description: reason ?? (chosen.length > 0 ? `Ask ${joinNames(chosen.map(formatCandidate))} to review #${pullRequest.number}` : `Ask for a review of #${pullRequest.number}`),
    onSelect: open,
  }
}

/** Why the reviewers of an open pull request can't be asked now; null when they can */
export function getRequestReviewBlocker(branch: Pick<Branch, "remoteStatus">, pullRequest: PullRequest): string | null {
  if (pullRequest.isDraft) {
    return `#${pullRequest.number} is a draft: mark it ready for review on GitHub first`
  }
  const push = getPushAction(branch)
  // the remote status not loaded yet is no reason
  if (push !== null && push !== "upToDate") {
    return "Push first, so the reviewers see the commits"
  }
  return null
}

/**
 * The toast of a request GitHub asked someone in: "Requested a review from Ilya Korennoy on #1078", a `warning` whose
 * description names whom GitHub refused ("GitHub refused Bob: …"); null when it asked nobody (an error, `formatRefused`
 * says why). `nameOf` gives the full name of a login the popover knows.
 */
export function formatRequestedToast(pullRequest: Pick<PullRequest, "number">, result: RequestedReviewers, nameOf: (login: string) => string): Omit<ToastMessage, "action"> | null {
  if (result.requested.length === 0) {
    return null
  }
  const title = `Requested a review from ${joinNames(result.requested.map(nameOf))} on #${pullRequest.number}`
  if (result.refused.length === 0) {
    return { title }
  }
  return { type: "warning", title, description: result.refused.map(refused => `GitHub refused ${nameOf(refused.login)}: ${refused.reason}`).join(" · ") }
}

/** Whom GitHub refused, with its reason: "Bob: Review cannot be requested from pull request author." */
export function formatRefused(result: RequestedReviewers, nameOf: (login: string) => string): string {
  return result.refused.map(refused => `${nameOf(refused.login)}: ${refused.reason}`).join("; ")
}
