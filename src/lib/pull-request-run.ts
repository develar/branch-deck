// The creation of the pull request of a subject and a review request of its open one, from every surface that offers
// them (the "Create pull request" dialog of the main window and of the review window, Request Review…): the intents
// `CreatePullRequest` and `RequestReview` Rust plans and runs (`operations::pull_request`;
// spec/branch/pull-requests.spec.md). Each pushes first what the remote lacks of the subject (the push of its push button,
// the stack branch of a whole stack), in the repository the user asked in, whichever the main window shows meanwhile;
// the creation waits for a mirror that copies the pushed branch a moment later, "Continue on GitHub" until GitHub has the
// branch, whose form the page opens then. The close of a pull request the user asks (`ClosePullRequest`). The page only
// words how it went.
import type { Intent, PullRequest, RequestedReviewers, RunningOperation, Subject } from "@/lib/bindings"
import { formatCreatedToast } from "@/lib/create-pull-request"
import { commandFailure } from "@/lib/errors"
import type { CompareFields } from "@/lib/pull-request"
import { getCompareUrl } from "@/lib/pull-request"
import type { CreatePullRequestContext, CreatePullRequestInput } from "@/lib/pull-request-dialog"
import type { IntentRun } from "@/lib/run-intent"
import { runFailure, runIntent } from "@/lib/run-intent"
import type { ToastMessage } from "@/lib/toast-message"
import { formatReadinessSubject, stackReadinessKey, stackRootOfReadinessKey } from "@/lib/whole-stack"
import { openNewPullRequest } from "@/presenters/pull-request-url"
import { forgetLandedStackRoot } from "@/stores/landed-stacks"

/** The subject of the row `key`: a branch, or the whole stack of a root (`stackReadinessKey`) */
function subjectOf(key: string): Subject {
  const root = stackRootOfReadinessKey(key)
  return root === null ? { kind: "branch", name: key } : { kind: "wholeStack", root }
}

/** The row of `subject`: the branch, `stackReadinessKey(root)` of a whole stack */
function keyOf(subject: Subject): string {
  return subject.kind === "branch" ? subject.name : stackReadinessKey(subject.root)
}

/** The row of the dialog of `context`: the branch, or the whole stack of its root */
function contextKey(context: CreatePullRequestContext): string {
  return context.wholeStack === null ? context.branchName : stackReadinessKey(context.branchName)
}

/** Why a run did not get to its end, as an error to throw (`runFailure`); null when it did */
function failureOf(run: IntentRun, noPlan: string): Error | null {
  const failure = runFailure(run, noPlan)
  if (failure === null) {
    return null
  }
  return typeof failure === "string" ? new Error(failure) : commandFailure(failure)
}

/**
 * The run of the creation of the pull request of `context` (`onGitHub`: "Continue on GitHub"), with the push it makes
 * first on the push button of its row; throws why it did not get to its end
 */
async function runCreation(context: CreatePullRequestContext, input: Omit<CreatePullRequestInput, "viewerLogin">, onGitHub: boolean): Promise<IntentRun> {
  const intent: Intent = {
    kind: "createPullRequest",
    subject: subjectOf(contextKey(context)),
    title: input.title,
    body: input.body,
    assignSelf: input.assignSelf,
    reviewers: [...input.reviewers],
    onGitHub,
  }
  const run = await runIntent(context.repositoryPath, intent)
  const failure = failureOf(run, `The last sync has no ${context.label}`)
  if (failure !== null) {
    throw failure
  }
  return run
}

/**
 * Creates the pull request of the dialog of `context` with `input` (`CreatePullRequest`: the push of what it needs
 * first, then GitHub, which finds the one it has of the head): the toast to show, with the pull request it opens; the
 * journal of the repository has a new one for Activity. Throws why nothing was created, or what GitHub said
 */
export async function submitPullRequest(
  context: CreatePullRequestContext,
  input: CreatePullRequestInput,
): Promise<{ toast: Omit<ToastMessage, "action">; pullRequest: PullRequest }> {
  const run = await runCreation(context, input, false)
  const created = run.kind === "ran" && run.outcome.result.kind === "pullRequest" ? run.outcome.result : null
  if (created === null) {
    throw new Error(`GitHub opened no pull request of ${context.label}`)
  }
  if (context.wholeStack !== null) {
    // a new pull request of the stack matches the stack as it is
    forgetLandedStackRoot(context.branchName, context.repositoryPath, context.branchPrefix)
  }
  const toast = formatCreatedToast(context.label, context.base, input.viewerLogin, created)
  return { toast, pullRequest: created.pullRequest }
}

/**
 * "Continue on GitHub": pushes what the pull request of `context` needs and waits until GitHub has the branch
 * (`CreatePullRequest` with `onGitHub`: GitHub's form of a branch it doesn't have shows nothing), then opens GitHub's
 * form with `fields`. Throws why nothing was pushed, or that GitHub still doesn't have the branch
 */
export async function continueOnGitHub(context: CreatePullRequestContext, fields: CompareFields): Promise<void> {
  if (context.repository === null || context.newPullRequestBlocker !== null) {
    throw new Error(context.newPullRequestBlocker ?? "The GitHub repository of the branches is unknown")
  }
  const repository = context.repository
  await runCreation(context, { title: fields.title ?? "", body: fields.body ?? "", reviewers: [], assignSelf: fields.assignee !== undefined }, true)
  openNewPullRequest(getCompareUrl(repository, context.base, context.head, fields))
}

/**
 * Asks `logins` to review the open pull request of the row `key` (a branch, `stackReadinessKey(root)`) in
 * `repositoryPath` (`RequestReview`, the push of the work the remote lacks first, so the reviewers see the commits):
 * whom GitHub asked and whom it refused, also when it asked nobody. Throws why nothing was asked
 */
export async function requestReview(key: string, logins: readonly string[], repositoryPath: string): Promise<RequestedReviewers> {
  const intent: Intent = { kind: "requestReview", subject: subjectOf(key), reviewers: [...logins], pushFirst: true }
  const run = await runIntent(repositoryPath, intent)
  if (run.kind === "ran" && run.outcome.result.kind === "reviewers") {
    return run.outcome.result
  }
  throw failureOf(run, `${formatReadinessSubject(key)} has no open pull request`) ?? new Error(`Nobody was asked to review the pull request of ${formatReadinessSubject(key)}`)
}

/** The running creation (`createPullRequest`) or review request (`requestReview`) of the row `key` among `operations` */
export function pullRequestOperationIn(
  operations: Readonly<Record<string, RunningOperation>>,
  kind: "createPullRequest" | "requestReview",
  key: string,
): RunningOperation | undefined {
  return Object.values(operations).find(operation => {
    const intent = operation.intent
    return (intent.kind === "createPullRequest" || intent.kind === "requestReview") && intent.kind === kind && keyOf(intent.subject) === key
  })
}

/**
 * Closes the pull request `number` of `repositoryPath` with `comment` (`ClosePullRequest`: one already closed gets no
 * second comment), the session has it closed once this resolves. Throws why it was not closed, what GitHub said
 */
export async function closePullRequest(repositoryPath: string, number: number, comment: string): Promise<void> {
  const run = await runIntent(repositoryPath, { kind: "closePullRequest", number, comment })
  const failure = failureOf(run, `The repository lists no pull request #${number}`)
  if (failure !== null) {
    throw failure
  }
}
