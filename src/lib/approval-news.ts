import type { News, Person } from "@/lib/bindings"
import { joinNames } from "@/lib/text"
import { formatReadinessSubject } from "@/lib/whole-stack"
import { formatPerson } from "@/presenters/next-step"

/** The approvals that made a waiting branch ready: "Ilya Korennoy approved wsm-skip-wa-2" */
export interface ApprovalNews {
  /** The branch, or the key of a whole stack (`stackReadinessKey`) */
  readonly branchName: string
  readonly pullRequest: number
  /** Whose approval counts now in the groups that waited; empty when Patronus doesn't name them */
  readonly approvers: readonly Person[]
  /** The groups that waited, in the order of the readiness */
  readonly groups: readonly string[]
}

/**
 * The approvals of the news of a session (`approvalsArrived`, which `domain_policy::news::approval_news` decides:
 * a subject that waited for approvals in the pull request of the same tip is ready after a new review), each news once;
 * the other news are the robots'
 */
export function approvalNewsOf(news: readonly News[]): ApprovalNews[] {
  return news.flatMap(({ event }) =>
    event.kind === "approvalsArrived" ? [{ branchName: event.subject, pullRequest: event.pullRequest, approvers: event.approvers, groups: event.groups }] : [],
  )
}

/**
 * The words of the toast, the OS notification and the Activity entry: "Ilya Korennoy approved wsm-skip-wa-2" (several:
 * "Ilya Korennoy and Anna Ivanova approved …"; Patronus naming nobody: "The code owners approved …"; the pull request of
 * a whole stack: "… approved the whole air stack"), "#1078 · IJPL Code Platform"
 */
export function describeApprovalNews(news: ApprovalNews): { title: string; description: string } {
  const who = news.approvers.length === 0 ? "The code owners" : joinNames(news.approvers.map(formatPerson))
  return {
    title: `${who} approved ${formatReadinessSubject(news.branchName)}`,
    description: news.groups.length === 0 ? `#${news.pullRequest}` : `#${news.pullRequest} · ${joinNames(news.groups)}`,
  }
}
