import { toast } from "@/components/ui/toast"
import { formatIde, formatRefusal, openDraftInAir } from "@/lib/air-launch"
import { commands } from "@/lib/bindings"
import type { AgentCliCommand, AirDraftAnswer, PatronusRobot, RobotFailureReport } from "@/lib/bindings"
import { hostOf } from "@/lib/cli-tools"
import { copyTextLater } from "@/lib/clipboard"
import { commandFailure, notifyError } from "@/lib/errors"
import { formatRobotSubject } from "@/lib/patronus"
import { repositoryName } from "@/lib/repository-config"
import { buildRobotPrompt } from "@/lib/robot-prompt"
import { openSettings } from "@/lib/settings-window"
import { ACTION_TOAST_TIMEOUT_MS, showToast, TOAST_TIMEOUT_MS } from "@/lib/toast-message"
import { branchSyncStore, structureIn } from "@/stores/branch-sync"
import { patronusRequest } from "@/stores/patronus"
import { getEffectiveBranchPrefix } from "@/stores/repository"

/** How long the "Copied" toast stays: the default of a toast without an action; the loading one had none */
const COPIED_TOAST_TIMEOUT_MS = 5000

/** The prompt for an agent and the report it tells */
export interface RobotPrompt {
  prompt: string
  report: RobotFailureReport
}

/**
 * The branches the robot's branch is built on, root first (`BranchStructure.ancestors`): the robot of a stacked branch builds the
 * whole stack, so the commit map has their commits too; empty for a root branch. For the robot of the whole stack, the
 * branches of the stack, the root first: its stack branch holds the commits of them all.
 */
export function robotAncestors(robot: PatronusRobot): string[] {
  const state = branchSyncStore.getState()
  if (robot.branchName === null) {
    return robot.stackRoot == null ? [] : [...(state.stackBranches[robot.stackRoot]?.members ?? [])]
  }
  return [...structureIn(state, robot.branchName).ancestors]
}

async function readAgentCliCommand(repositoryPath: string): Promise<AgentCliCommand | null> {
  const result = await commands.agentCliCommand(repositoryPath)
  // a failure of the read: the prompt says it without bd
  return result.status === "ok" ? result.data : null
}

/**
 * The failure of the robot with the commit map of its branch, and the prompt built from it. The fast version reads the
 * details the app already loaded (cached by the backend once the robot finished); the full one also reads the failed
 * builds, their tests and logs.
 */
export async function readRobotPrompt(robot: PatronusRobot, full: boolean): Promise<RobotPrompt> {
  const { repositoryPath } = patronusRequest()
  // the prompt names the branches as the page shows them
  const branchPrefix = getEffectiveBranchPrefix()
  const baselineBranch = branchSyncStore.getState().baselineBranch ?? ""
  const [result, bd] = await Promise.all([
    commands.getRobotFailure({ repositoryPath, robotId: robot.id, branchName: robot.branchName, ancestors: robotAncestors(robot), baselineBranch, full }),
    readAgentCliCommand(repositoryPath),
  ])
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  const report = result.data
  return { prompt: buildRobotPrompt({ repositoryPath, branchPrefix, baselineBranch, report, bd }), report }
}

function branchLabel(robot: PatronusRobot): string {
  return formatRobotSubject(robot)
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

/** "Copy Prompt" of a toast of Start in Air: the toast closes, and a toast says the copy, as the clipboard shows nothing */
function copyInstead(id: string, robot: PatronusRobot): void {
  toast.close(id)
  void copyRobotPrompt(robot).then(copied => {
    if (copied) {
      showToast({ title: `Copied the prompt for ${branchLabel(robot)}`, description: "Paste it into your coding agent" })
    }
  })
}

/**
 * "Start in Air": opens the New Session tab with the fast prompt in the IDE with Air that has the repository open, with
 * the default agent of its New Session button (openspec/specs/branch/start-in-air/spec.md). The IDE may ask the user
 * first, so a toast says it opens, and turns in place into what the IDE answered: opened, no IDE with Copy Prompt, or
 * the refusal in its words
 */
export async function startRobotInAir(robot: PatronusRobot): Promise<void> {
  const { repositoryPath } = patronusRequest()
  const branch = branchLabel(robot)
  // the consent dialog of the IDE can take up to 120 s
  const id = toast.add({ type: "loading", title: "Opening in Air…", description: `New Session for ${branch}`, timeout: 0 })
  let answer: AirDraftAnswer
  try {
    const { prompt } = await readRobotPrompt(robot, false)
    answer = await openDraftInAir(repositoryPath, prompt)
  } catch (error) {
    toast.close(id)
    notifyError(`Cannot open the New Session for ${branch} in Air`, error)
    return
  }
  switch (answer.status) {
    case "opened":
      toast.update(id, { type: "success", title: `Opened in ${formatIde(answer.ide)}`, description: `New Session for ${branch}`, timeout: TOAST_TIMEOUT_MS })
      return
    case "noIde":
      toast.update(id, {
        type: "info",
        title: `No IDE with Air has ${repositoryName(repositoryPath)} open`,
        description: "Open the repository in an IDE with Air and try again, or copy the prompt",
        timeout: ACTION_TOAST_TIMEOUT_MS,
        actionProps: { children: "Copy Prompt", onClick: () => copyInstead(id, robot) },
      })
      return
    case "refused":
      toast.update(id, {
        type: "error",
        title: `Cannot open the New Session for ${branch} in Air`,
        description: formatRefusal(answer.code, answer.message),
        timeout: ACTION_TOAST_TIMEOUT_MS,
      })
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
  // set by the read, which the copy waits for
  const read = { contents: "", unavailableServer: null as string | null }
  try {
    await copyTextLater(
      readRobotPrompt(robot, true).then(({ prompt, report }) => {
        read.contents = formatFullPromptContents(report)
        read.unavailableServer = report.failure.teamcityUnavailableAt ?? null
        return prompt
      }),
    )
  } catch (error) {
    toast.close(id)
    notifyCopyFailed(robot, error)
    return
  }
  const { contents, unavailableServer: server } = read
  if (server === null) {
    toast.update(id, { type: "success", title: `Copied the prompt for ${branch}`, description: contents, timeout: COPIED_TOAST_TIMEOUT_MS })
    return
  }
  // the CLI that reads the build logs is missing or not signed in to the server of a build: the way to it
  toast.update(id, {
    type: "success",
    title: `Copied the prompt for ${branch}`,
    description: `${contents} · no build logs: teamcity is not signed in to ${hostOf(server)}`,
    timeout: ACTION_TOAST_TIMEOUT_MS,
    actionProps: { children: "Set Up TeamCity CLI", onClick: () => openSettings({ page: "tools", focus: "cli-teamcity", server }) },
  })
}
