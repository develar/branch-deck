/**
 * The actions of a subject as Rust decides them (`RepositoryView.actions`, `domain_policy::actions`) in the page's look
 * and words: each action by its stable id has one look (`ACTION_LOOKS`: its label where no fact names it, its icon, its
 * key), and a blocker one sentence (`formatActionBlocker`). Every surface builds its items here (`presentAction`): the
 * "⋯" and right-click menus, the row's keys, the branch preview, the review window, the stack island and the command
 * palette, so an action is offered, named and refused alike everywhere. What runs (an operation of the view, a rewrite of
 * the page, a push) lies over an item (`operationItem`); so does a sync of the table over a rewrite (`whileSyncing`),
 * which queues behind it then. The actions of a robot run in the session (`getRobotActionItems`): a
 * restart or a cancel that runs there is running on every surface.
 */
import type { LucideIcon } from "lucide-react"
import {
  ArchiveXIcon,
  CirclePauseIcon,
  CirclePlayIcon,
  ClipboardCopyIcon,
  ClipboardListIcon,
  CombineIcon,
  DownloadIcon,
  EraserIcon,
  ExternalLinkIcon,
  FileDiffIcon,
  FlaskConicalIcon,
  FolderInputIcon,
  GitBranchIcon,
  GitMergeIcon,
  GitPullRequestArrowIcon,
  GitPullRequestCreateIcon,
  GitPullRequestIcon,
  Layers2Icon,
  LayersIcon,
  ListXIcon,
  MessageSquareDiffIcon,
  PencilIcon,
  PenLineIcon,
  RefreshCwIcon,
  RotateCcwIcon,
  ShieldCheckIcon,
  ShieldOffIcon,
  SlidersHorizontalIcon,
  TagIcon,
  UngroupIcon,
  UploadIcon,
  UserCheckIcon,
  UserPlusIcon,
  CheckIcon,
} from "lucide-react"
import type { ActionBlocker, ActionId, ActionState, NothingBeyond, PatronusRobot, RunningOperation } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem } from "@/lib/menu"
import { formatPushMode, formatRobotRestartLabel, formatRobotSubject, openRobot } from "@/lib/patronus"
import { openPullRequestUrl } from "@/lib/pull-request"
import { copyRobotFullPrompt, copyRobotPrompt } from "@/lib/robot-prompt-copy"
import { cancelRobot, forgetStart, openUnknownStart, restartRobot } from "@/lib/robot-run"
import { formatAskFirst } from "@/lib/stack"
import { getRowPart } from "@/model/store"
import { formatPendingFact, formatProblemsFailure, formatStepBlocker } from "@/presenters/next-step"
import { formatLanded, formatReason, formatRobotsLanded, formatStackHold } from "@/presenters/reasons"
import { branchSyncStore } from "@/stores/branch-sync"

/** How an action looks on every surface: its label where no fact names it ("Push (2)" does), its icon, its key */
export interface ActionLook {
  readonly label: string
  readonly icon: LucideIcon
  readonly kbds?: readonly string[]
}

/** The look of every action Rust offers, by id */
export const ACTION_LOOKS = {
  "safe-push": { label: "Safe Push", icon: ShieldCheckIcon, kbds: ["s"] },
  "safe-push-with-profile": { label: "Safe Push With Profile…", icon: SlidersHorizontalIcon },
  "dry-run": { label: "Dry Run", icon: FlaskConicalIcon },
  "check-approvals": { label: "Check Approvals Again", icon: RefreshCwIcon },
  "safe-push-without-review": { label: "Safe Push Without Review…", icon: ShieldOffIcon },
  push: { label: "Push", icon: UploadIcon, kbds: ["p"] },
  "push-with-children": { label: "Push with Children", icon: UploadIcon },
  "pick-remote-commits": { label: "Pick Remote Commits…", icon: DownloadIcon },
  "review-changes": { label: "Review Changes", icon: FileDiffIcon, kbds: ["d"] },
  "open-pull-request": { label: "Open Pull Request", icon: GitPullRequestIcon, kbds: ["o"] },
  "request-review": { label: "Request Review…", icon: UserPlusIcon },
  "retarget-pull-request": { label: "Retarget", icon: GitPullRequestArrowIcon },
  "create-pull-request": { label: "Create Pull Request…", icon: GitPullRequestCreateIcon },
  rename: { label: "Rename…", icon: PencilIcon },
  "move-to": { label: "Move to…", icon: FolderInputIcon, kbds: ["m"] },
  "stack-on": { label: "Stack On…", icon: LayersIcon },
  unstack: { label: "Unstack", icon: Layers2Icon },
  ungroup: { label: "Ungroup", icon: UngroupIcon },
  "amend-changes": { label: "Amend Changes…", icon: PenLineIcon },
  "add-issue-reference": { label: "Add Issue Reference…", icon: TagIcon },
  "squash-fixups": { label: "Squash Fixups", icon: CombineIcon },
  unapply: { label: "Unapply…", icon: ArchiveXIcon },
  park: { label: "Park Branch…", icon: CirclePauseIcon },
  unpark: { label: "Unpark Branch", icon: CirclePlayIcon },
  "safe-push-whole-stack": { label: "Safe Push Whole Stack", icon: ShieldCheckIcon },
  "dry-run-whole-stack": { label: "Dry Run Whole Stack", icon: FlaskConicalIcon },
  "safe-push-main-line": { label: "Safe Push", icon: ShieldCheckIcon },
  "open-stack-pull-request": { label: "Open Pull Request", icon: GitPullRequestIcon },
  "create-stack-pull-request": { label: "Create Pull Request of the Stack…", icon: GitPullRequestCreateIcon },
  "review-whole-stack": { label: "Review Whole Stack", icon: FileDiffIcon },
  "restart-robot": { label: "Restart Safe Push", icon: RotateCcwIcon },
  "push-and-restart-robot": { label: "Restart Safe Push", icon: RotateCcwIcon },
  "cancel-robot": { label: "Cancel Safe Push", icon: ShieldOffIcon },
  "copy-robot-prompt": { label: "Copy Prompt for Agent", icon: ClipboardCopyIcon },
  "copy-robot-full-prompt": { label: "Copy Prompt with CI Details", icon: ClipboardListIcon },
  "open-robot": { label: "Open in Patronus", icon: ExternalLinkIcon },
  "open-robot-pull-request": { label: "Open Pull Request", icon: GitPullRequestIcon },
  review: { label: "Review", icon: UserCheckIcon },
  approve: { label: "Approve", icon: CheckIcon },
  "safe-push-pull-request": { label: "Approve and Safe Push…", icon: GitMergeIcon },
  "request-changes": { label: "Request Changes…", icon: MessageSquareDiffIcon },
  "show-branch": { label: "Show Branch", icon: GitBranchIcon },
  "remove-from-reviews": { label: "Remove from Reviews", icon: ListXIcon },
  "open-unknown-start": { label: "Open in Patronus", icon: ExternalLinkIcon },
  "forget-start": { label: "Forget This Start", icon: EraserIcon },
} as const satisfies Record<ActionId, ActionLook>

/** What the words of a blocker read beside it, of the page: the subject it refuses and what the page shows of it */
export interface BlockerWords {
  /** The branch the action acts on (of a whole stack its root) */
  readonly subject: string
  /** The status text of the row of the branch (`Branch.statusText`): why its sync failed */
  readonly statusText?: string
  /**
   * The words of a reason of the next step of the subject (`BranchNextStep.safePushBlocker`, `dryRunBlocker`, the
   * blocker of its start without review): the step words its own reasons by its kind
   */
  readonly step?: string | null
}

/** Why Push with Children of `name` pushes nothing beyond the branch itself */
function nothingBeyondWords(name: string, why: NothingBeyond): string {
  switch (why.kind) {
    case "askFirst":
      return formatAskFirst(why.names)
    case "onlyBranch":
      return `Only ${name} has commits to push: its own push sends them`
    case "landed":
      return `${formatRobotsLanded(why.by, why.own, why.target)}: nothing of ${name} and its children is left to push`
    case "stackBranch":
      switch (why.along.kind) {
        case "notBuilt":
          return `The branches of the stack are on the remote. ${formatReason(why.along.reason)}`
        case "held":
          return `The branches of the stack are on the remote. ${formatStackHold(why.along.reason)}`
        case "dropsRemote":
          return "The branches of the stack are on the remote. The whole stack branch stays: its push would drop remote commits it lacks"
        default:
          return "The branches of the stack are on the remote. Its stack branch goes there with the pull request or the Safe Push of the whole stack"
      }
    case "onRemote":
      return "Nothing to push: the stack is on the remote"
  }
}

/** Why the branch failed to sync, as the action `id` says it: the push names the status of its row, a rewrite its stack problem */
function syncFailedWords(id: ActionId, problem: string | null, words: BlockerWords): string {
  if (id === "push") {
    const status = words.statusText ?? ""
    return status === "" ? "The branch failed to sync" : `The branch failed to sync: ${status}`
  }
  return problem ?? "The branch failed to sync"
}

/** "The branch has no commits", as the action `id` says it */
function noCommitsWords(id: ActionId): string {
  switch (id) {
    case "push":
      return "No commits to push"
    case "review-changes":
    case "review-whole-stack":
      return "No commits to review"
    default:
      return "The branch has no commits"
  }
}

/** The words of a sync of the table that a rewrite waits for */
export const SYNC_WAIT = "Wait for the sync to finish"

/** Why nobody is asked to review a draft (`PullRequestView.draft`) */
export function formatDraftReason(number: number): string {
  return `#${number} is a draft: mark it ready for review on GitHub first`
}

type BlockerFormatters = {
  readonly [C in ActionBlocker["code"]]: (blocker: Extract<ActionBlocker, { code: C }>, id: ActionId, words: BlockerWords) => string
}

const BLOCKERS = {
  rule: ({ reason }, _id, words) => formatReason(reason, () => words.statusText ?? ""),
  step: ({ blocker }, _id, words) => words.step ?? formatStepBlocker(blocker),
  pending: ({ fact }) => formatPendingFact(fact),
  problemsFailed: ({ message }) => formatProblemsFailure(message),
  syncing: () => SYNC_WAIT,
  syncFailed: ({ problem }, id, words) => syncFailedWords(id, problem, words),
  noCommits: (_blocker, id) => noCommitsWords(id),
  upToDate: () => "The remote branch is up to date",
  draft: ({ number }) => formatDraftReason(number),
  commentRoute: ({ number }) => `A comment on #${number} runs the default checks: it can't choose a profile`,
  allReferenced: () => "Every commit of the branch has an issue reference",
  parkHeld: ({ reason }) => `${formatReason(reason)}: it lands whatever a park says`,
  nothingBeyond: ({ why }, _id, words) => nothingBeyondWords(words.subject, why),
  landed: ({ landed }) => formatLanded(landed),
  ownPullRequest: () => "Your own pull request: GitHub takes no review of its author",
  approved: ({ commit }) => `You approved ${formatShortHash(commit)}`,
  targetUnread: () => "Reading the branch Patronus pushes into…",
  noTarget: () => "Patronus pushes into a branch of origin: the baseline is none",
  otherBase: ({ number, base, target }) => `#${number} merges into ${base}: Patronus would merge it there, not into ${target}`,
} satisfies BlockerFormatters as BlockerFormatters

/** The page's words of why the action `id` can't run now */
export function formatActionBlocker(id: ActionId, blocker: ActionBlocker, words: BlockerWords): string {
  return (BLOCKERS[blocker.code] as (blocker: ActionBlocker, id: ActionId, words: BlockerWords) => string)(blocker, id, words)
}

/** The state of the action `id` among `actions` (a subject's, `RepositoryView.actions`); undefined while it isn't offered */
export function findAction(actions: readonly ActionState[] | undefined, id: ActionId): ActionState | undefined {
  return actions?.find(action => action.id === id)
}

/** Why the action `id` can't run now, in the page's words; null while it can, undefined while the subject doesn't offer it */
export function actionBlocker(actions: readonly ActionState[] | undefined, id: ActionId, words: BlockerWords): string | null | undefined {
  const action = findAction(actions, id)
  if (action === undefined) {
    return undefined
  }
  return action.availability.kind === "enabled" ? null : formatActionBlocker(id, action.availability.reason, words)
}

/** The subject offers the action `id` and it can run now */
export function isActionEnabled(actions: readonly ActionState[] | undefined, id: ActionId): boolean {
  return findAction(actions, id)?.availability.kind === "enabled"
}

/** What a surface gives an action beside its look: what it runs, and what the facts name of it */
export interface ActionItem {
  readonly onSelect: MenuItem["onSelect"]
  /** The label the facts name ("Push (2)", "Unstack from auth"); the look's otherwise */
  readonly label?: string
  /** What it does, shown while it can run */
  readonly description?: string
  /** The keys of the surface; the look's otherwise, none where the key doesn't work there */
  readonly kbds?: readonly string[] | null
  readonly children?: MenuItem[]
  readonly compactLabel?: string
}

/**
 * The item of the action `id` of a subject (`actions`): its look, what the surface gives, disabled with the words of why
 * it can't run; null while the subject doesn't offer it
 */
export function presentAction(actions: readonly ActionState[] | undefined, id: ActionId, item: ActionItem, words: BlockerWords): MenuItem | null {
  const blocker = actionBlocker(actions, id, words)
  if (blocker === undefined) {
    return null
  }
  const look: ActionLook = ACTION_LOOKS[id]
  const kbds = item.kbds === null ? undefined : (item.kbds ?? look.kbds)
  const description = blocker ?? item.description
  return {
    id,
    label: item.label ?? look.label,
    icon: look.icon,
    onSelect: item.onSelect,
    ...(kbds !== undefined && { kbds: [...kbds] }),
    ...(blocker !== null && { disabled: true }),
    ...(description !== undefined && { description }),
    ...(item.children !== undefined && { children: item.children }),
    ...(item.compactLabel !== undefined && { compactLabel: item.compactLabel }),
  }
}

/**
 * A rewrite while a sync of the table runs: it waits for the sync (a quiet resync of a row that shows its last state
 * too), which Rust doesn't send with every row; an item disabled already keeps its reason
 */
export function whileSyncing(item: MenuItem, syncing: boolean): MenuItem {
  return syncing && item.disabled !== true ? { ...item, disabled: true, description: SYNC_WAIT } : item
}

/** An action of a robot (`getRobotActionItems`), named by its `id` */
export interface RobotAction extends MenuItem {
  id: "restart-robot" | "push-and-restart-robot" | "cancel-robot" | "copy-robot-prompt" | "copy-robot-full-prompt" | "open-robot" | "open-robot-pull-request"
}

/** The operations of the session of the repository the main window shows, by id (`RepositoryView.operations`) */
export type SessionOperations = Readonly<Record<string, RunningOperation>>

/** The operation of the session that restarts (`restartRobot`) or cancels (`cancelRobot`) the robot `robotId` */
function robotOperationIn(running: SessionOperations, kind: "restartRobot" | "cancelRobot", robotId: string): RunningOperation | undefined {
  return Object.values(running).find(operation => operation.intent.kind === kind && operation.intent.robotId === robotId)
}

/** What each action of a robot runs and how the facts of the robot name it */
function robotActionItem(robot: PatronusRobot, id: RobotAction["id"]): ActionItem {
  const mode = formatPushMode(robot.pushMode)
  switch (id) {
    case "restart-robot":
      return { label: formatRobotRestartLabel(robot), onSelect: () => void restartRobot(robot) }
    case "push-and-restart-robot": {
      // a running robot tests a commit the branch moved past: the restart cancels it first, the one push into its
      // branch; the push is part of the restart (its plan in Rust), its name is the restart's
      const pushed = robot.branchName ?? formatRobotSubject(robot)
      const runs = robot.status === "running" && robot.sourceRevision !== null
      return {
        label: formatRobotRestartLabel(robot),
        description: runs
          ? `Cancels the run on ${formatShortHash(robot.sourceRevision!)}, pushes ${pushed} and starts it again on the pushed commit`
          : `Pushes ${pushed} and starts it again on the pushed commit`,
        onSelect: () => void restartRobot(robot),
      }
    }
    case "cancel-robot":
      return { label: `Cancel ${mode}`, onSelect: () => void cancelRobot(robot) }
    case "copy-robot-prompt":
      return { onSelect: () => void copyRobotPrompt(robot) }
    case "copy-robot-full-prompt":
      return { onSelect: () => void copyRobotFullPrompt(robot) }
    case "open-robot":
      return { onSelect: () => openRobot(robot) }
    case "open-robot-pull-request": {
      const pullRequest = robot.pullRequest
      return { label: `Open Pull Request #${pullRequest?.number ?? ""}`, kbds: null, onSelect: () => pullRequest != null && openPullRequestUrl(pullRequest.url) }
    }
  }
}

/** The item of a restart or a cancel while the session runs it: running, a second click joins it in Rust */
function whileRunning(item: MenuItem, robot: PatronusRobot, id: RobotAction["id"], running: SessionOperations): MenuItem {
  const kind = id === "cancel-robot" ? "cancelRobot" : id === "restart-robot" || id === "push-and-restart-robot" ? "restartRobot" : null
  if (kind === null || robotOperationIn(running, kind, robot.id) === undefined) {
    return item
  }
  const mode = formatPushMode(robot.pushMode)
  return { ...item, running: true, disabled: true, description: kind === "cancelRobot" ? `Canceling the ${mode}…` : `Restarting the ${mode}…` }
}

/** What the items of the actions of a robot read beside the robot */
export interface RobotItemsOf {
  /** Why the step of its branch holds a restart back (the words of that step: approvals it misses) */
  readonly stepReason?: string | null
  /** The actions of the robot a component subscribed to (`robot:<id>`), so its list follows them */
  readonly offered?: readonly ActionState[] | undefined
  /** The operations of the session a component subscribed to: a restart or a cancel that runs */
  readonly running?: SessionOperations
}

/**
 * The actions of a robot as Rust offers them (`robot:<id>` of `RepositoryView.actions`, `domain_policy::actions`), one
 * list for every surface that shows it (the branch menu, the robot details, the robots indicator, a toast), in this
 * order: "Restart Safe Push" (it pushes first while the branch has work the remote lacks, also for a running robot,
 * which it cancels first), "Cancel Safe Push" while it runs, "Copy Prompt for Agent" and "Copy Prompt with CI Details"
 * for a failed or failing robot, "Open in Patronus", and "Open Pull Request #12" for a robot of a pull request. Rust
 * disables a restart and the prompts while Patronus didn't tell why the robot failed, a restart while the approvals the
 * step of its branch misses (`stepReason`) or what refuses it stands in its way. A restart and a cancel run in the
 * session (`restartRobot`, `cancelRobot`): running there, their items are running on every surface.
 */
export function getRobotActionItems(robot: PatronusRobot, of: RobotItemsOf = {}): RobotAction[] {
  const actions = (of.offered === undefined ? getRowPart(`robot:${robot.id}`, "actions") : of.offered) ?? []
  const running = of.running ?? branchSyncStore.getState().operations
  const words = { subject: robot.branchName ?? robot.stackRoot ?? "", step: of.stepReason ?? null }
  return actions.flatMap(action => {
    const id = action.id as RobotAction["id"]
    const item = presentAction(actions, id, robotActionItem(robot, id), words)
    return item === null ? [] : [whileRunning(item, robot, id, running) as RobotAction]
  })
}

/** An action of a start whose answer was lost (`getStartActionItems`), named by its `id` */
export interface StartAction extends MenuItem {
  id: "open-unknown-start" | "forget-start"
}

/**
 * The actions of a start whose answer was lost as Rust offers them (`offered`: `op:<id>` of `RepositoryView.actions`,
 * `domain_policy::actions::start_actions`, which a component subscribed to), beside its row's slot: "Open in Patronus" (of a comment start "Open Pull
 * Request #7") where its robot would be, and "Forget This Start", which lets the subject start again while Patronus may
 * still run the robot it sent; none for an operation that runs
 */
export function getStartActionItems(operation: RunningOperation, offered: readonly ActionState[] | undefined): StartAction[] {
  const actions = offered ?? []
  const subject = operation.intent.kind === "land" ? (operation.intent.subject.kind === "branch" ? operation.intent.subject.name : operation.intent.subject.root) : ""
  const step = operation.step.kind === "startUnknown" ? operation.step : null
  return actions.flatMap(action => {
    const id = action.id as StartAction["id"]
    const item: ActionItem =
      id === "open-unknown-start"
        ? {
            ...(step?.pullRequest != null && { label: `Open Pull Request #${step.pullRequest}` }),
            kbds: null,
            onSelect: () => openUnknownStart(operation),
          }
        : {
            description: `Lets ${subject} start again. Patronus may still run the start that was sent: its robot shows once Patronus lists it`,
            onSelect: () => void forgetStart(operation),
          }
    const presented = presentAction(actions, id, item, { subject })
    return presented === null ? [] : [presented as StartAction]
  })
}

/** The restart of a robot, which pushes first or not */
export function isRestart(item: RobotAction): boolean {
  return item.id === "restart-robot" || item.id === "push-and-restart-robot"
}
