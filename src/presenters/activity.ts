// The words of Activity (spec/repository/activity.spec.md): an entry of a journal in the words its toast had, never a
// rule. An operation of the remote (`JournalEntry`, of the app or of `bd`) is an entry per event it made: each branch it
// pushed (`describePush` over the status the push found, `EffectRecord.remote`) or found moved on the remote, each pull
// request it retargeted, the robot it started, restarted or canceled (`presentLandOutcome`), the pull request it
// created, closed or reviewed; one that failed is an error entry unless the page recorded the error of its toast (an
// entry of the page that names the operation), which shows where the operation ran: in the projects of its journal,
// whichever repository the main window showed when the toast came. An operation of an agent (its worktree a slot of the
// lease pool) says so (`agent`): Activity shows it behind its Agents chip. An entry the page recorded (`PageEntry`) is as
// the page worded it.
import type { ActivityLog, ApiError, EffectRecord, Intent, JournalEntry, PageEntry, RemoteStatusUpdate } from "@/lib/bindings"
import { formatCreatedToast } from "@/lib/create-pull-request"
import { describeRobot, formatPushMode } from "@/lib/patronus"
import type { GitHubRepository } from "@/lib/pull-request"
import { describePush } from "@/lib/push-summary"
import { buildRemoteMovedTexts, toRemoteStatus } from "@/lib/remote-status"
import { describeReview } from "@/lib/review-requests"
import { formatReadinessSubject, formatWholeStackBranch, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { presentLandOutcome, presentLandRefusal } from "@/presenters/outcome"
import { presentRewrite } from "@/presenters/rewrite"
import type { ActivityDetails, ActivityEntry } from "@/stores/activity"

/** What an event of an operation says, before it is placed in a repository and in time */
type Said = Omit<ActivityEntry, "id" | "repositoryPath" | "time" | "operationId" | "agent">

/**
 * The entries of `log`, newest first (in one second the later line of a journal first, and an entry of the page before
 * an operation): each operation of a journal for each recent project whose repository it is the journal of, each entry of
 * the page in its repository, or one that names an operation of a journal in the projects of that journal. A pull
 * request links in `github`, the GitHub repository of the selected repository
 */
export function presentActivity(log: ActivityLog, github: GitHubRepository | null): ActivityEntry[] {
  const told = new Set(log.page.flatMap(entry => (entry.activity.operationId === null ? [] : [entry.activity.operationId])))
  // the projects of each operation of a journal: where what the page told of it shows
  const projectsOf = new Map<string, readonly string[]>()
  const entries: ActivityEntry[] = []
  for (const journal of log.journals) {
    for (const entry of journal.entries) {
      projectsOf.set(entry.id, journal.repositories)
      const said = presentOperation(entry, github, told.has(entry.id) || toldElsewhere(entry, told))
      const agent = entry.leaseSlot ? { agent: { worktree: entry.worktree } } : {}
      for (const repositoryPath of journal.repositories) {
        said.forEach((it, at) => entries.push({ ...it, id: `${entry.id}/${at}`, repositoryPath, time: entry.endedAt * 1000, operationId: entry.id, ...agent }))
      }
    }
  }
  for (const page of log.page) {
    entries.push(...placed(pageEntry(page), projectsOf))
  }
  return entries
    .map((entry, at) => ({ entry, at }))
    .sort((a, b) => b.entry.time - a.entry.time || b.at - a.at)
    .map(it => it.entry)
}

/** An entry of the page as it recorded it */
function pageEntry({ id, endedAt, activity }: PageEntry): ActivityEntry {
  return {
    id,
    repositoryPath: activity.repositoryPath,
    time: endedAt * 1000,
    kind: activity.kind,
    title: activity.title,
    ...(activity.description !== null && { description: activity.description }),
    ...(activity.warning === true && { warning: true }),
    ...(activity.details !== null && { details: activity.details }),
    ...(activity.operationId !== null && { operationId: activity.operationId }),
  }
}

/**
 * An entry of the page where it shows: in its repository, or, when it names an operation of a journal (the error of its
 * toast), in the projects of that journal, each with an id of its own
 */
function placed(entry: ActivityEntry, projectsOf: ReadonlyMap<string, readonly string[]>): ActivityEntry[] {
  const projects = entry.operationId === undefined ? undefined : projectsOf.get(entry.operationId)
  if (projects === undefined) {
    return [entry]
  }
  return projects.map((repositoryPath, at) => ({ ...entry, id: at === 0 ? entry.id : `${entry.id}/${at}`, repositoryPath }))
}

/** "https://github.com/acme/app/pull/12": a pull request of the repository on github.com */
function pullRequestDetails(github: GitHubRepository | null, number: number): ActivityDetails | undefined {
  return github === null ? undefined : { type: "pullRequest", url: `https://github.com/${github.owner}/${github.name}/pull/${number}` }
}

/**
 * The events of the operation `entry`: what each effect that went did, what its family answered, and why it failed
 * (`told`: the page recorded the error of its toast)
 */
function presentOperation(entry: JournalEntry, github: GitHubRepository | null, told: boolean): Said[] {
  // a start whose answer was lost, settled or forgotten long after its operation
  if (entry.transition !== null) {
    return []
  }
  const said: Said[] = []
  for (const record of entry.effects) {
    said.push(...effectSaid(record, entry, github))
  }
  said.push(...resultSaid(entry, github, told))
  return said
}

/** The branch a push sends as its toast names it: of a whole stack its stack branch */
function pushedName(key: string): string {
  const root = stackRootOfReadinessKey(key)
  return root === null ? key : formatWholeStackBranch(root)
}

/** The push of `record`: each branch in the words of its toast, or the branch whose remote moved, nothing pushed */
function pushSaid(record: EffectRecord, entry: JournalEntry): Said[] {
  if (record.effect.kind !== "push") {
    return []
  }
  const remote: readonly (RemoteStatusUpdate | null)[] = record.remote ?? []
  if (record.status.kind === "failed") {
    const moved = record.status.error.code === "lease_rejected" && entry.intent.kind === "push" && entry.intent.scope.kind === "alone" ? remote[0] : null
    if (moved == null) {
      return []
    }
    const texts = buildRemoteMovedTexts(record.effect.branches[0] ?? "", toRemoteStatus(moved))
    return [{ kind: "push", title: texts.title, description: texts.description, warning: true }]
  }
  if (record.status.kind !== "done") {
    return []
  }
  return record.effect.branches.map((key, at) => {
    const status = remote[at] ?? null
    const before = status === null ? null : toRemoteStatus(status)
    const news = describePush(pushedName(key), before)
    const dropped = before?.droppedCommits ?? []
    const details: ActivityDetails | undefined =
      news.warning !== true || before === null || dropped.length === 0
        ? undefined
        : { type: "dropped", commits: [...dropped], more: Math.max(0, before.commitsBehind - dropped.length) }
    return { kind: "push", ...news, ...(details !== undefined && { details }) }
  })
}

/** A base as the rows name it: a virtual branch of `prefix` by its short name (`auth` of `user/virtual/auth`) */
function shortBase(base: string, prefix: string): string {
  const virtual = `${prefix}/virtual/`
  return base.startsWith(virtual) ? base.slice(virtual.length) : base
}

/** What the effect of `record` did, when it went: a push, a retarget, a close of the user, a review */
function effectSaid(record: EffectRecord, entry: JournalEntry, github: GitHubRepository | null): Said[] {
  const { effect } = record
  if (effect.kind === "push") {
    return pushSaid(record, entry)
  }
  if (record.status.kind !== "done") {
    return []
  }
  const linked = (number: number) => pullRequestDetails(github, number)
  switch (effect.kind) {
    case "retarget":
      return [
        {
          kind: "review",
          title: `Retargeted #${effect.number} to ${effect.to}`,
          description: `${effect.branch} · it went into ${shortBase(effect.from, entry.prefix)}`,
          details: linked(effect.number),
        },
      ]
    case "closePullRequest": {
      if (entry.intent.kind !== "closePullRequest") {
        // the closes of a merged robot are states of its merge
        return []
      }
      const reason = effect.comment.replace(/^Closed in Branch Deck: /, "").replace(/\.$/, "")
      return [{ kind: "review", title: `Closed pull request #${effect.number}`, description: reason, details: linked(effect.number) }]
    }
    case "submitReview":
      return [{ kind: "review", title: describeReview(effect.verdict, effect.number), details: linked(effect.number) }]
    default:
      return []
  }
}

/** "the whole air stack", "auth": the subject of an operation as a sentence names it */
function subjectOf(entry: JournalEntry): string {
  return entry.subject === null ? "the branch" : formatReadinessSubject(entry.subject)
}

/** What the family of the operation answered: the robot it started, restarted or canceled, the pull request it created */
function resultSaid(entry: JournalEntry, github: GitHubRepository | null, told: boolean): Said[] {
  const { intent, result } = entry
  if (intent.kind === "land") {
    return landSaid(entry, told)
  }
  const said: Said[] = []
  if (result.kind === "robot" && result.robot !== null) {
    const mode = formatPushMode(result.robot.pushMode)
    const title = intent.kind === "cancelRobot" ? (result.alreadyFinished ? `${mode} already finished` : `${mode} is canceling`) : `${mode} restarted`
    said.push(robotSaid(title, result.robot))
  } else if (result.kind === "push" && result.restarted !== null) {
    said.push(robotSaid(`${formatPushMode(result.restarted.pushMode)} restarted`, result.restarted))
  } else if (result.kind === "pullRequest" && result.created) {
    const toast = formatCreatedToast(subjectOf(entry), result.pullRequest.baseRefName ?? "", null, result)
    said.push({
      kind: "review",
      title: toast.title,
      ...(toast.description !== undefined && { description: toast.description }),
      ...(toast.type === "warning" && { warning: true }),
      details: { type: "pullRequest", url: result.pullRequest.url },
    })
  } else if (result.kind === "review") {
    if (result.robot !== null) {
      said.push(robotSaid("Safe Push started", result.robot))
    } else if (result.comment?.accepted === true) {
      said.push({ kind: "patronus", title: `Safe Push started on #${result.comment.number}`, details: pullRequestDetails(github, result.comment.number) })
    }
  }
  const failure = told ? null : failureOf(entry)
  if (failure !== null) {
    said.push({ kind: "error", title: failureTitle(intent, subjectOf(entry)), description: failure })
  }
  return said
}

function robotSaid(title: string, robot: Parameters<typeof describeRobot>[0]): Said {
  return { kind: "patronus", title, description: describeRobot(robot), details: { type: "robot", url: robot.url } }
}

/** A start of a Safe Push or a Dry Run, as its toast said it (`presentLandOutcome`, `presentLandRefusal`) */
function landSaid(entry: JournalEntry, told: boolean): Said[] {
  const { intent } = entry
  if (intent.kind !== "land") {
    return []
  }
  const subject = subjectOf(entry)
  if (entry.error !== null) {
    if (told || entry.error.code === "canceled") {
      return []
    }
    const refusal = presentLandRefusal(entry.error, intent.mode, subject)
    return [
      { kind: "error", title: refusal.title, description: refusal.kind === "landing" ? refusal.reason : refusal.kind === "stopped" ? refusal.description : refusal.error.message },
    ]
  }
  const toast = presentLandOutcome(entry, intent, subject)
  switch (toast.kind) {
    case "started":
      return [{ kind: "patronus", title: toast.title, description: toast.description, details: { type: "robot", url: toast.robot.url } }]
    case "commented":
      return [{ kind: "patronus", title: toast.title, description: toast.description, details: { type: "pullRequest", url: toast.url } }]
    case "unknown":
      return [
        {
          kind: "patronus",
          title: toast.title,
          description: toast.description,
          warning: true,
          ...(toast.url !== null && { details: { type: "pullRequest" as const, url: toast.url } }),
        },
      ]
    case "failed":
      return told ? [] : [{ kind: "error", title: toast.title, description: toast.description }]
    case "stopped":
      return []
  }
}

/** Why the operation did not get to what it was for: its error, or the first effect that failed (a retarget's is told on the way) */
function failureErrorOf(entry: JournalEntry): ApiError | null {
  if (entry.error !== null) {
    return entry.error
  }
  for (const { effect, status } of entry.effects) {
    if (status.kind === "failed" && effect.kind !== "retarget" && !(effect.kind === "push" && status.error.code === "lease_rejected")) {
      return status.error
    }
  }
  return null
}

/** The words of `failureErrorOf`; none for a stopped wait, which changed nothing */
function failureOf(entry: JournalEntry): string | null {
  const error = failureErrorOf(entry)
  return error === null || error.code === "canceled" ? null : error.message
}

/**
 * Whether the page told the failure of the operation under the id of the operation it came from (the fetch of an update
 * runs as an operation of its own, whose error the update's toast tells)
 */
function toldElsewhere(entry: JournalEntry, told: ReadonlySet<string>): boolean {
  const id = failureErrorOf(entry)?.operation_id
  return id != null && told.has(id)
}

/** "Cannot push auth", "Cannot restart the robot": what a failed operation was for */
function failureTitle(intent: Intent, subject: string): string {
  switch (intent.kind) {
    case "push":
      return `Cannot push ${subject}`
    case "restartRobot":
      return "Cannot restart the robot"
    case "cancelRobot":
      return "Cannot cancel the robot"
    case "retarget":
      return `Cannot retarget pull request #${intent.number}`
    case "createPullRequest":
      return `Cannot create the pull request of ${subject}`
    case "requestReview":
      return `Cannot request a review of ${subject}`
    case "closePullRequest":
      return `Cannot close pull request #${intent.number}`
    case "closeLandedPullRequests":
      return "Cannot close the pull requests of the merged robot"
    case "submitReview":
      return `Cannot review pull request #${intent.number}`
    case "setIssueState":
      return `Cannot change ${intent.key}`
    case "forgetStart":
      return "Cannot forget the start"
    case "land":
      return `Cannot start the ${formatPushMode(intent.mode)} of ${subject}`
    case "rewrite":
      return presentRewrite(intent).failure
    case "fetchBaseline":
      return "Cannot fetch the baseline"
    case "updateProject":
      return "Cannot update the project"
    case "dismissLanded":
      return "Cannot dismiss the landed commits"
    case "fetchReview":
      return `Cannot fetch pull request #${intent.number}`
  }
}
