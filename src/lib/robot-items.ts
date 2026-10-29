import type { PatronusRobot } from "@/lib/bindings"
import { getRowPart } from "@/model/store"
import type { RobotAction } from "@/presenters/actions"
import { getRobotActionItems } from "@/presenters/actions"
import { getOperationWords } from "@/stores/indicators"

/**
 * The actions of `robot` as Rust offers them now (`getRobotActionItems` of `robot:<id>` of the view, in the words of the
 * operations of the session), for what is built on demand: a menu on open, a toast, the step of a toast
 */
export function getRobotActionItemsNow(robot: PatronusRobot): RobotAction[] {
  return getRobotActionItems(robot, { ...getOperationWords(), offered: getRowPart(`robot:${robot.id}`, "actions") })
}
