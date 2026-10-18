import { Fragment } from "react"
import type { ReactNode } from "react"
import { toast } from "@/components/ui/toast"
import type { PatronusRobot } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import type { Retargeted } from "@/lib/git/new-pull-request"
import { getPullRequestRetarget, getPullRequestToastAction, getRetargetCommand } from "@/lib/git/new-pull-request"
import { formatPushMode, openRobot } from "@/lib/patronus"
import type { BranchPush, PushNews } from "@/lib/push-summary"
import { describePush, describeStackPush } from "@/lib/push-summary"
import type { ToastActionProps } from "@/lib/toast-message"
import { ACTION_TOAST_TIMEOUT_MS } from "@/lib/toast-message"
import type { Branch } from "@/stores/branch-sync"
import { getBranch } from "@/stores/branch-sync"

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

function addPushToast(title: string, clauses: readonly Clause[], action: ToastActionProps | null): void {
  toast.add({
    type: "success",
    title,
    description: renderClauses(clauses),
    ...(action !== null && { timeout: ACTION_TOAST_TIMEOUT_MS, actionProps: action }),
  })
}

/**
 * The toast of a push of a branch (`news`, as Activity says it), read after the push: the remote branch it created can
 * have a pull request now. It offers the pull request: open it, create it (into the baseline, as every pull request), or
 * copy the `gh` command that retargets one into a virtual branch (the push retargets it first, `retargeted`; the command
 * is for a retarget that failed). A push that restarted a robot says so and opens the robot.
 */
export function showPushToast(branchName: string, news: PushNews, restarted?: PatronusRobot, retargeted?: Retargeted): void {
  const branch = getBranch(branchName)
  const retargetCommand = branch === undefined ? null : getRetargetCommand(branch)
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
  const action: ToastActionProps | null =
    restarted !== undefined ? { children: "Open in Patronus", onClick: () => openRobot(restarted) } : branch === undefined ? null : getPushAction(branch, retargetCommand)
  addPushToast(news.title, clauses, action)
}

/**
 * The one toast of a push of several branches of a stack, "Pushed auth, login and oauth" (Activity has an entry per
 * branch): what the pushes did, a pull request of them that needs a retarget, the robots they restarted; the pull
 * request action is of `startedFrom`, the branch the user pushed the stack from, unless a retarget comes first
 */
export function showStackPushToast(pushes: readonly PushOutcome[], startedFrom: string): void {
  const [first] = pushes
  if (first === undefined) {
    return
  }
  if (pushes.length === 1) {
    showPushToast(first.name, describePush(first.name, first.before), first.restarted, first.retargeted)
    return
  }

  const news = describeStackPush(pushes)
  const clauses: Clause[] = news.description === undefined ? [] : [{ text: news.description }]
  for (const push of pushes) {
    if (push.retargeted !== undefined) {
      clauses.push({ ...retargetedClause(push.retargeted), text: `retargeted pull request #${push.retargeted.number} of ${push.name} to ` })
    }
  }
  let retargeted: { branch: Branch; command: string } | null = null
  for (const push of pushes) {
    const branch = getBranch(push.name)
    const command = branch === undefined ? null : getRetargetCommand(branch)
    if (branch !== undefined && command !== null) {
      retargeted = { branch, command }
      clauses.push(describeRetargetNeeded(branch, branch.name)!)
      break
    }
  }
  for (const push of pushes) {
    if (push.restarted !== undefined) {
      clauses.push(restartClause(push.restarted, push.name))
    }
  }
  const started = getBranch(startedFrom)
  const action = retargeted !== null ? getPushAction(retargeted.branch, retargeted.command) : started === undefined ? null : getPullRequestToastAction(started)
  addPushToast(news.title, clauses, action)
}
