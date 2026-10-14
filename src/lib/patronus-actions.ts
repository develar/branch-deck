import { toast } from "@/components/ui/toast"
import type { PatronusRobot } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"

/** Runs a restart or a cancel of a robot from a menu or a popover, and says how it went */
export async function runRobotAction(action: (robot: PatronusRobot) => Promise<PatronusRobot>, robot: PatronusRobot, success: string, failure: string): Promise<void> {
  try {
    const result = await action(robot)
    toast.add({ type: "success", title: success, description: result.name })
  }
  catch (error) {
    notifyError(failure, error)
  }
}
