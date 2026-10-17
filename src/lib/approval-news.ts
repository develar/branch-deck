import type { Person, ReviewReadiness } from "@/lib/bindings"
import { formatPerson } from "@/lib/branch-next-step"
import { joinNames } from "@/lib/text"

/** A readiness of a branch as the store kept it: its tip and the pull request it counts the approvals of */
export interface ReadinessSnapshot {
  readonly tip: string
  /** The pull request the robot would start from; null for a start through the API */
  readonly pullRequest: number | null
  readonly readiness: ReviewReadiness
}

/** The approvals that made a waiting branch ready: "Ilya Korennoy approved wsm-skip-wa-2" */
export interface ApprovalNews {
  readonly branchName: string
  readonly pullRequest: number
  /** Whose approval counts now in the groups that waited; empty when Patronus doesn't name them */
  readonly approvers: readonly Person[]
  /** The groups that waited, in the order of the readiness */
  readonly groups: readonly string[]
}

/**
 * News when the approvals of a branch arrived: before, Patronus missed approvals in the pull request; now, for the same
 * tip and the same pull request (a new review, not a new commit of the user), it misses none. Null otherwise: the first
 * answer of a session (an approval made while the app was closed is no news, the row shows it), a start through the API,
 * another tip.
 */
export function findApprovalNews(branchName: string, previous: ReadinessSnapshot | undefined, next: ReadinessSnapshot): ApprovalNews | null {
  if (previous === undefined || previous.readiness.kind !== "missing" || next.pullRequest === null) {
    return null
  }
  if (previous.tip !== next.tip || previous.pullRequest !== next.pullRequest) {
    return null
  }
  if (next.readiness.kind !== "approved" && next.readiness.kind !== "notRequired") {
    return null
  }
  const waited = previous.readiness.groups.map(group => group.name)
  const approved = next.readiness.kind === "approved" ? next.readiness.groups : []
  const approvers: Person[] = []
  for (const group of approved) {
    if (!waited.includes(group.name)) {
      continue
    }
    for (const person of group.approvedBy) {
      if (!approvers.some(it => samePerson(it, person))) {
        approvers.push(person)
      }
    }
  }
  return { branchName, pullRequest: next.pullRequest, approvers, groups: waited }
}

function samePerson(a: Person, b: Person): boolean {
  const key = (person: Person) => (person.login ?? person.fullName ?? "").toLowerCase()
  return key(a) === key(b)
}

/** Each approval is told once: the branch, the pull request and whose approval */
export function approvalNewsKey(news: ApprovalNews): string {
  const approvers = news.approvers.map(person => (person.login ?? person.fullName ?? "").toLowerCase()).sort()
  return JSON.stringify([news.branchName, news.pullRequest, approvers])
}

/**
 * The words of the toast, the OS notification and the Activity entry: "Ilya Korennoy approved wsm-skip-wa-2" (several:
 * "Ilya Korennoy and Anna Ivanova approved …"; Patronus naming nobody: "The code owners approved …"), "#1078 · IJPL
 * Code Platform"
 */
export function describeApprovalNews(news: ApprovalNews): { title: string; description: string } {
  const who = news.approvers.length === 0 ? "The code owners" : joinNames(news.approvers.map(formatPerson))
  return { title: `${who} approved ${news.branchName}`, description: news.groups.length === 0 ? `#${news.pullRequest}` : `#${news.pullRequest} · ${joinNames(news.groups)}` }
}
