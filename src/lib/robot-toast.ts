import type { PatronusRobot } from "@/lib/bindings"
import type { EventToastAction, EventToastState } from "@/lib/event-toast"
import { isEventToastOpen, replaceEventToast, showEventToast, toastKeys } from "@/lib/event-toast"
import type { MenuItem } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import { describeRobot, formatPushMode, formatRobotSubject, openRobot } from "@/lib/patronus"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore } from "@/stores/branch-sync"
import { indicatorsStore } from "@/stores/indicators"
import { getBranchRobot, getStackRobot, patronusStore } from "@/stores/patronus"
import { getSelectedRepositoryPath, isSelectedRepository, repositoryStore } from "@/stores/repository"

/*
 * The toast of a robot (docs/guidelines/ux-writing.md#one-toast-per-event): its start, restart and cancel, a required
 * check that failed while it runs, its failure, its merge (`src/lib/landing-toast.ts`) are the states of one toast of
 * the robot, and a newer robot of the same subject and push mode (a restart, a new start) replaces it. Its step is the
 * step of the robot as the stores have it, rendered again on every change: Cancel Safe Push leaves the toast once the
 * robot finished, a restart elsewhere turns it into "Safe Push of auth was restarted". A state of an older robot never
 * takes the place of the toast of a newer one of its subject: the failure of the robot a start replaced, told after the
 * start (its problems read first), leaves the start's toast as it is, and Activity records it.
 */

/** What the robot tests, as the keys of its toasts name it: the branch, the whole stack, else the robot itself */
export function robotToastSubject(robot: Pick<PatronusRobot, "id" | "branchName" | "stackRoot">): string {
  return robot.branchName ?? (robot.stackRoot == null ? `robot:${robot.id}` : stackReadinessKey(robot.stackRoot))
}

/**
 * The keys of the toast of a robot: its own, and the subject's in its push mode, which a newer robot takes over; a Safe
 * Push takes over the toast of the approvals that it starts on too
 */
export function robotToastKeys(repositoryPath: string, robot: PatronusRobot): { key: string; aliases: string[]; supersedes: string[] } {
  const subject = robotToastSubject(robot)
  const ofSubject = toastKeys.robotSubject(repositoryPath, subject, robot.pushMode)
  const approval = robot.pushMode === "dryRun" ? [] : [toastKeys.approval(repositoryPath, subject)]
  return { key: toastKeys.robot(repositoryPath, robot.id), aliases: [ofSubject], supersedes: [ofSubject, ...approval] }
}

/** By the key of the robots of a subject in one push mode, the newest robot whose toast showed under it */
const newestShown = new Map<string, PatronusRobot>()

/** Whether the open toast of the robots of the subject (`subjectKey`) tells a newer robot than `robot`: a restart, a start that replaced it */
function isReplaced(subjectKey: string, robot: PatronusRobot): boolean {
  const newest = newestShown.get(subjectKey)
  return newest !== undefined && newest.id !== robot.id && newest.startTime > robot.startTime && isEventToastOpen(subjectKey)
}

/** The robot of the id as a store has it now, in any repository */
export function currentRobot(id: string): PatronusRobot | undefined {
  for (const repository of Object.values(patronusStore.getState().repositories)) {
    const robot = repository.robots[id]
    if (robot !== undefined) {
      return robot
    }
  }
  return undefined
}

/**
 * A newer robot of the same subject and push mode in the repository the main window shows (a restart, made here or
 * elsewhere); null when there is none, or the robot is of another repository
 */
function newerRobotOf(repositoryPath: string, robot: PatronusRobot): PatronusRobot | null {
  if (!isSelectedRepository(repositoryPath)) {
    return null
  }
  const latest = robot.branchName !== null ? getBranchRobot(robot.branchName) : robot.stackRoot != null ? getStackRobot(robot.stackRoot) : undefined
  return latest !== undefined && latest.id !== robot.id && latest.pushMode === robot.pushMode && latest.startTime > robot.startTime ? latest : null
}

/** "Safe Push of auth was restarted" / "auth → master", which opens the newer robot */
export function formatRestartedRobot(robot: PatronusRobot, newer: PatronusRobot): { title: string; description: string } {
  return { title: `${formatPushMode(robot.pushMode)} of ${formatRobotSubject(robot)} was restarted`, description: describeRobot(newer) }
}

export interface RobotToast {
  readonly repositoryPath: string
  readonly robot: PatronusRobot
  readonly type: EventToastState["type"]
  readonly title: string
  readonly description: string
  /**
   * The step of the robot and its branch as the stores have them now (no branch once the main window shows another
   * repository, whose branch of the same name is another one); none, or a disabled one, offers nothing
   */
  readonly stepOf?: (current: PatronusRobot, branch: Branch | null) => MenuItem | null
  /** The step after "Open in Patronus", as the Cancel of a start; by default it comes first */
  readonly stepLast?: boolean
  /** Only in place of the open toast of the robot, as a state no news of its own (a failure told failing before) */
  readonly replaceOnly?: boolean
}

/** The step of a menu as the action of a toast: running while its operation runs, none while disabled */
function stepAction(step: MenuItem | null): EventToastAction | undefined {
  if (step === null) {
    return undefined
  }
  if (step.running === true) {
    return { label: step.label, running: true }
  }
  return step.disabled === true ? undefined : { label: step.label, onClick: () => step.onSelect(FROM_MENU) }
}

/** Shows the toast of a robot, which follows the robot from then on; nothing while a newer robot of its subject holds the subject's toast */
export function showRobotEventToast({ repositoryPath, robot, type, title, description, stepOf, stepLast = false, replaceOnly = false }: RobotToast): void {
  const keys = robotToastKeys(repositoryPath, robot)
  const [subjectKey] = keys.aliases as [string]
  if (isReplaced(subjectKey, robot)) {
    return
  }
  newestShown.set(subjectKey, robot)
  const place = replaceOnly ? replaceEventToast : showEventToast
  place({
    ...keys,
    watch: [patronusStore, branchSyncStore, indicatorsStore, repositoryStore],
    render: () => {
      const current = currentRobot(robot.id) ?? robot
      const newer = newerRobotOf(repositoryPath, current)
      if (newer !== null) {
        return { type: "info", ...formatRestartedRobot(current, newer), action: { label: "Open in Patronus", onClick: () => openRobot(newer) } }
      }
      const branch = current.branchName === null || !isSelectedRepository(repositoryPath) ? null : (branchSyncStore.getState().branches[current.branchName] ?? null)
      const step = stepAction(stepOf?.(current, branch) ?? null)
      const patronus: EventToastAction = { label: "Open in Patronus", onClick: () => openRobot(current) }
      return stepLast || step === undefined
        ? { type, title, description, action: patronus, secondaryAction: step }
        : { type, title, description, action: step, secondaryAction: patronus }
    },
  })
}

/**
 * The toast of a robot that started, restarted or is canceling, in the repository the main window shows, where its
 * action ran ("Safe Push restarted" / "auth → master", with "Open in Patronus"): it takes the place of the toast of the
 * robot it restarted or cancels ("Safe Push of auth failed" becomes "Safe Push restarted")
 */
export function showRobotToast(title: string, robot: PatronusRobot, repositoryPath: string = getSelectedRepositoryPath()): void {
  showRobotEventToast({ repositoryPath, robot, type: "success", title, description: describeRobot(robot) })
}
