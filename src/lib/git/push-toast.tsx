import { Fragment } from "react"
import type { ReactNode } from "react"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { showEventToast, toastKeys } from "@/lib/event-toast"
import { formatPushMode, openRobot } from "@/lib/patronus"
import { openPullRequest } from "@/lib/pull-request"
import { getPullRequestRetarget, getPullRequestToastAction, getRetargetCommand } from "@/lib/pull-request-now"
import type { BranchPush, PushNews } from "@/lib/push-summary"
import { describePush, describeStackPush, formatPushedBranch } from "@/lib/push-summary"
import type { ToastActionProps } from "@/lib/toast-message"
import { formatWholeStackBranch, stackReadinessKey } from "@/lib/whole-stack"
import type { Retargeted } from "@/presenters/pull-request-url"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import { branchSyncStore, getBranch } from "@/stores/branch-sync"
import { pullRequestsStore } from "@/stores/pull-requests"
import { isSelectedRepository, repositoryStore } from "@/stores/repository"

/**
 * A push of a branch, the robot it restarted on the pushed commit (a Safe Push the push had to cancel), and the pull
 * request it retargeted first
 */
export interface PushOutcome extends BranchPush {
  restarted?: PatronusRobot
  retargeted?: Retargeted
}

/** A part of the description, " · " between them: the text, then a branch name in medium weight */
interface Clause {
  text: string
  name?: string
}

/**
 * What a push toast says about an open pull request of the branch into a virtual branch that the push didn't retarget
 * (the retarget failed): "the pull request goes into auth: retarget it to master"; null for one that needs no retarget.
 * `of` names the branch, for a toast of several.
 */
function describeRetargetNeeded(branch: Branch, of?: string): Clause | null {
  const retarget = getPullRequestRetarget(branch.name)
  if (retarget === null) {
    return null
  }
  return { text: `the pull request${of === undefined ? "" : ` of ${of}`} goes into ${retarget.from}: retarget it to `, name: retarget.base }
}

/** "Copy gh pr edit" for a pull request into a virtual branch whose retarget failed, else "Open Pull Request" or "Create Pull Request" */
function getPushAction(branch: Branch, retargetCommand: string | null): ToastActionProps | null {
  return retargetCommand === null ? getPullRequestToastAction(branch) : { children: "Copy gh pr edit", onClick: () => void copyText(retargetCommand) }
}

/** "retargeted pull request #8 to master" */
function retargetedClause(retargeted: Retargeted): Clause {
  return { text: `retargeted pull request #${retargeted.number} to `, name: retargeted.base }
}

function restartClause(robot: PatronusRobot, of?: string): Clause {
  return { text: `${formatPushMode(robot.pushMode)}${of === undefined ? "" : ` of ${of}`} restarted` }
}

function renderClauses(clauses: readonly Clause[]): ReactNode {
  if (clauses.length === 0) {
    return undefined
  }
  // the clauses of one toast never change
  return clauses.map((clause, index) => (
    <Fragment key={index}>
      {index === 0 ? clause.text.charAt(0).toUpperCase() + clause.text.slice(1) : ` · ${clause.text}`}
      {clause.name !== undefined && <span className="font-medium">{clause.name}</span>}
    </Fragment>
  ))
}

/**
 * The toast of a push into `repositoryPath`, one per pushed branch (`toastKeys.push`): a later push of the branch takes
 * its place, and its action is the one `actionOf` gives now, rendered again as the branches, the pull requests and the
 * shown repository change: "Create Pull Request…" becomes "Open Pull Request" once the branch has one, and the pull
 * request action leaves once the main window shows another repository
 */
function addPushToast(
  repositoryPath: string,
  branchNames: readonly [string, ...string[]],
  title: string,
  clauses: readonly Clause[],
  actionOf: () => ToastActionProps | null,
): void {
  const keys = branchNames.map(name => toastKeys.push(repositoryPath, name))
  // the clauses of one toast never change: one node, so a render that changes nothing keeps the toast as it is
  const description = renderClauses(clauses)
  showEventToast({
    key: keys[0]!,
    aliases: keys,
    supersedes: keys,
    watch: [branchSyncStore, pullRequestsStore, repositoryStore],
    render: () => {
      const action = actionOf()
      return { type: "success", title, description, ...(action !== null && { action: { label: action.children, onClick: action.onClick } }) }
    },
  })
}

/**
 * The branch of the table the toast of a push into `repositoryPath` reads after it: none once the main window shows
 * another repository, whose branch of the same name is another branch with its own pull request
 */
function pushedBranch(branchName: string, repositoryPath: string): Branch | undefined {
  return isSelectedRepository(repositoryPath) ? getBranch(branchName) : undefined
}

/**
 * The toast of a push of a branch into `repositoryPath` (`news`, as Activity says it), read after the push: the remote
 * branch it created can have a pull request now. It offers the pull request: open it, create it (into the baseline, as
 * every pull request), or copy the `gh` command that retargets one into a virtual branch (the push retargets it first,
 * `retargeted`; the command is for a retarget that failed). A push that restarted a robot says so and opens the robot.
 * A push that ends after a switch of the repository says what it pushed and offers no pull request.
 */
export function showPushToast(branchName: string, repositoryPath: string, news: PushNews, restarted?: PatronusRobot, retargeted?: Retargeted): void {
  const branch = pushedBranch(branchName, repositoryPath)
  const retargetNeeded = branch === undefined ? null : describeRetargetNeeded(branch)
  const clauses: Clause[] = []
  if (news.description !== undefined) {
    clauses.push({ text: news.description })
  }
  if (retargeted !== undefined) {
    clauses.push(retargetedClause(retargeted))
  } else if (retargetNeeded !== null) {
    clauses.push(retargetNeeded)
  }
  if (restarted !== undefined) {
    clauses.push(restartClause(restarted))
  }
  addPushToast(repositoryPath, [branchName], news.title, clauses, () => {
    if (restarted !== undefined) {
      return { children: "Open in Patronus", onClick: () => openRobot(restarted) }
    }
    const now = pushedBranch(branchName, repositoryPath)
    return now === undefined ? null : getPushAction(now, getRetargetCommand(now))
  })
}

/** The stack branch of a whole stack a push of its branches took along (Push with Children of the root) */
export interface StackBranchPushed {
  root: string
  /** The remote status it was pushed against */
  before: RemoteStatus | null
  /** The pull request of the whole stack, the action of a toast of the stack branch alone */
  pullRequest: PullRequest | undefined
}

/**
 * The one toast of a push of several branches of a stack, "Pushed branches auth, login and oauth" (Activity has an entry per
 * branch): what the pushes did, a pull request of them that needs a retarget, the robots they restarted; the pull
 * request action is of `startedFrom`, the branch the user pushed the stack from, unless a retarget comes first. The
 * stack branch of the whole stack that went along (`wholeStack`) is "pushed the whole stack branch"; pushed alone, the
 * toast is its own ("Pushed the whole stack branch of auth"), with the pull request of the whole stack. Pushed into
 * `repositoryPath`: after a switch of the repository it offers no pull request (`pushedBranch`).
 */
export function showStackPushToast(pushes: readonly PushOutcome[], startedFrom: string, repositoryPath: string, wholeStack: StackBranchPushed | null = null): void {
  const [first] = pushes
  if (first === undefined) {
    if (wholeStack !== null) {
      const news = describePush(formatWholeStackBranch(wholeStack.root), wholeStack.before)
      const pullRequest = wholeStack.pullRequest
      addPushToast(repositoryPath, [stackReadinessKey(wholeStack.root)], news.title, news.description === undefined ? [] : [{ text: news.description }], () =>
        pullRequest === undefined ? null : { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) },
      )
    }
    return
  }
  if (pushes.length === 1 && wholeStack === null) {
    showPushToast(first.name, repositoryPath, describePush(formatPushedBranch(first.name), first.before), first.restarted, first.retargeted)
    return
  }

  const news = describeStackPush(pushes, wholeStack !== null)
  const clauses: Clause[] = news.description === undefined ? [] : [{ text: news.description }]
  for (const push of pushes) {
    if (push.retargeted !== undefined) {
      clauses.push({ ...retargetedClause(push.retargeted), text: `retargeted pull request #${push.retargeted.number} of ${push.name} to ` })
    }
  }
  const retarget = findRetargetCommand(pushes, repositoryPath)
  if (retarget !== null) {
    clauses.push(describeRetargetNeeded(retarget.branch, retarget.branch.name)!)
  }
  for (const push of pushes) {
    if (push.restarted !== undefined) {
      clauses.push(restartClause(push.restarted, push.name))
    }
  }
  const names = [startedFrom, ...pushes.map(push => push.name).filter(name => name !== startedFrom)] as const
  addPushToast(repositoryPath, names, news.title, clauses, () => {
    const retargeted = findRetargetCommand(pushes, repositoryPath)
    if (retargeted !== null) {
      return getPushAction(retargeted.branch, retargeted.command)
    }
    const started = pushedBranch(startedFrom, repositoryPath)
    return started === undefined ? null : getPullRequestToastAction(started)
  })
}

/** The first pushed branch whose pull request goes into a virtual branch and needs a retarget, with its command */
function findRetargetCommand(pushes: readonly PushOutcome[], repositoryPath: string): { branch: Branch; command: string } | null {
  for (const push of pushes) {
    // the push retargeted it first: its pull request goes into the baseline, whatever the listing still says
    if (push.retargeted !== undefined) {
      continue
    }
    const branch = pushedBranch(push.name, repositoryPath)
    const command = branch === undefined ? null : getRetargetCommand(branch)
    if (branch !== undefined && command !== null) {
      return { branch, command }
    }
  }
  return null
}
