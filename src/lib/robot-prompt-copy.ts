import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import type { AgentCliCommand, PatronusRobot, RobotFailureReport } from "@/lib/bindings"
import { copyTextLater } from "@/lib/clipboard"
import { notifyError } from "@/lib/errors"
import { buildRobotPrompt } from "@/lib/robot-prompt"
import { stackAncestors } from "@/lib/stack"
import { branchSyncStore } from "@/stores/branch-sync"
import { patronusRequest } from "@/stores/patronus"

/** How long the "Copied" toast stays: the default of a toast without an action; the loading one had none */
const COPIED_TOAST_TIMEOUT_MS = 5000

/** The prompt for an agent and the report it tells */
export interface RobotPrompt {
  prompt: string
  report: RobotFailureReport
}

/**
 * The branches the robot's branch is built on, root first (`stackAncestors`): the robot of a stacked branch builds the
 * whole stack, so the commit map has their commits too; empty for a root branch
 */
export function robotAncestors(robot: PatronusRobot): string[] {
  if (robot.branchName === null) {
    return []
  }
  const branches = branchSyncStore.getState().branches
  return stackAncestors(robot.branchName, name => branches[name]).map(branch => branch.name)
}

async function readAgentCliCommand(repositoryPath: string): Promise<AgentCliCommand | null> {
  try {
    const result = await commands.agentCliCommand(repositoryPath)
    return result.status === "ok" ? result.data : null
  } catch {
    // the prompt says it without bd
    return null
  }
}

/**
 * The failure of the robot with the commit map of its branch, and the prompt built from it. The fast version reads the
 * details the app already loaded (cached by the backend once the robot finished); the full one also reads the failed
 * builds, their tests and logs.
 */
export async function readRobotPrompt(robot: PatronusRobot, full: boolean): Promise<RobotPrompt> {
  const { repositoryPath, branchPrefix } = patronusRequest()
  const baselineBranch = branchSyncStore.getState().baselineBranch ?? ""
  const [result, bd] = await Promise.all([
    commands.getRobotFailure({ repositoryPath, branchPrefix, robotId: robot.id, branchName: robot.branchName, ancestors: robotAncestors(robot), baselineBranch, full }),
    readAgentCliCommand(repositoryPath),
  ])
  if (result.status === "error") {
    throw new Error(result.error.message)
  }
  const report = result.data
  return { prompt: buildRobotPrompt({ repositoryPath, branchPrefix, baselineBranch, report, bd }), report }
}

function branchLabel(robot: PatronusRobot): string {
  return robot.branchName ?? robot.name
}

/** "Cannot copy the prompt for auth" */
function notifyCopyFailed(robot: PatronusRobot, error: unknown): void {
  notifyError(`Cannot copy the prompt for ${branchLabel(robot)}`, error)
}

/** Copies the fast prompt, as a menu item copies: no toast, an error only */
export async function copyRobotPrompt(robot: PatronusRobot): Promise<boolean> {
  try {
    await copyTextLater(readRobotPrompt(robot, false).then(({ prompt }) => prompt))
    return true
  } catch (error) {
    notifyCopyFailed(robot, error)
    return false
  }
}

/** "3 failed tests · log of 2 builds", of what the full prompt holds */
export function formatFullPromptContents(report: RobotFailureReport): string {
  const builds = report.failure.builds ?? []
  const tests = builds.reduce((sum, build) => sum + build.failedTests.length, 0) + report.failure.omitted.tests
  const logs = builds.filter(build => build.logTail !== null).length
  const parts = [...(tests === 0 ? [] : [`${tests} failed test${tests === 1 ? "" : "s"}`]), ...(logs === 0 ? [] : [`log of ${logs} build${logs === 1 ? "" : "s"}`])]
  return parts.length === 0 ? `${report.failure.checks.length} failed check${report.failure.checks.length === 1 ? "" : "s"}` : parts.join(" · ")
}

/**
 * Collects the CI details and copies the full prompt. It takes seconds, so a toast says it collects, and turns in place
 * into "Copied the prompt for auth" with what it holds
 */
export async function copyRobotFullPrompt(robot: PatronusRobot): Promise<void> {
  const branch = branchLabel(robot)
  const id = toast.add({ type: "loading", title: `Collecting CI details of ${branch}…`, timeout: 0 })
  let contents = ""
  try {
    await copyTextLater(
      readRobotPrompt(robot, true).then(({ prompt, report }) => {
        contents = formatFullPromptContents(report)
        return prompt
      }),
    )
  } catch (error) {
    toast.close(id)
    notifyCopyFailed(robot, error)
    return
  }
  toast.update(id, { type: "success", title: `Copied the prompt for ${branch}`, description: contents, timeout: COPIED_TOAST_TIMEOUT_MS })
}
