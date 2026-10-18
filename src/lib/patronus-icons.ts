import type { ComponentType } from "react"
import { ShieldAlertIcon, ShieldCheckIcon, ShieldEllipsisIcon, ShieldIcon, ShieldOffIcon, ShieldXIcon } from "lucide-react"
import type { PatronusChipKind } from "@/lib/patronus"

/** The shield of each robot state: the chip, the Safe Push slot of the row, the details and the landing overview */
export const patronusChipIcons: Readonly<Record<PatronusChipKind, ComponentType<{ className?: string }>>> = {
  running: ShieldEllipsisIcon,
  failed: ShieldXIcon,
  needsReview: ShieldAlertIcon,
  merged: ShieldCheckIcon,
  passed: ShieldCheckIcon,
  canceled: ShieldOffIcon,
  unknown: ShieldIcon,
}
