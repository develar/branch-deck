// The start that waited for a merge (`afterLanding` of the next step of a subject: "Lands after auth"), kept from the
// toast of the merge (`showMergeToast`) until the update that lands it in the target, whose toast takes the place of the
// merge's and offers it ("Start ui"): one toast per event, never a second one, and nothing starts by itself
import type { NextStepView } from "@/lib/bindings"
import type { EventToastAction } from "@/lib/event-toast"

const waiting = new Map<string, EventToastAction>()

/**
 * The subject whose start waits for each running robot (`afterLanding` of its next step, "Lands after auth"), by the
 * robot's id: the one that lands the most. Noted from every view of the steps while the robot runs (`applyView`), as
 * the step of a dependent of a merged robot says ready, not "after", by the time the merge is told
 */
const waitingOn = new Map<string, { readonly key: string; readonly lands: number }>()

/** Notes the subjects of `nextSteps` (the view's, by key) whose start waits for a running robot */
export function noteWaitingStarts(nextSteps: Readonly<Record<string, NextStepView>>): void {
  for (const [key, view] of Object.entries(nextSteps)) {
    const kind = view.step.kind
    if (kind.kind !== "afterLanding") {
      continue
    }
    const known = waitingOn.get(kind.robot.robotId)
    if (known === undefined || known.key === key || view.step.lands > known.lands) {
      waitingOn.set(kind.robot.robotId, { key, lands: view.step.lands })
    }
  }
}

/** The subject whose start waited for the robot `robotId`, once; null when none did */
export function takeWaitingStart(robotId: string): string | null {
  const key = waitingOn.get(robotId)?.key ?? null
  waitingOn.delete(robotId)
  return key
}

/** The start `action` of the subject that waited for a merge in `repositoryPath`, offered once its update lands it */
export function keepStartAfterMerge(repositoryPath: string, action: EventToastAction): void {
  waiting.set(repositoryPath, action)
}

/** The start kept for `repositoryPath`, once: the toast of the update that landed the merge offers it; null without one */
export function takeStartAfterMerge(repositoryPath: string): EventToastAction | null {
  const action = waiting.get(repositoryPath) ?? null
  waiting.delete(repositoryPath)
  return action
}
