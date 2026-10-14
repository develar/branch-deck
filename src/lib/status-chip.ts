import type { UIColor } from "@/lib/ui-types"

/** The hues of a status chip (pull request, Patronus robot) */
export type StatusChipColor = Extract<UIColor, "success" | "error" | "info" | "warning" | "neutral">

export const statusChipTextColors: Readonly<Record<StatusChipColor, string>> = {
  success: "text-success",
  error: "text-destructive",
  info: "text-info",
  warning: "text-warning",
  neutral: "text-muted-foreground",
}
