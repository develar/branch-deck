import { cn } from "cn"
import { ProgressLine } from "@/components/shared/ProgressLine"

/**
 * The rotating ring of a running Patronus robot: a track and an arc, slower than a spinner (a robot runs for minutes).
 * Sized by its `Button` (`data-icon`) or by `className`; the colour is the text colour. `around` is the ring around the
 * node of the stack graph: wider and thinner, so the node keeps its gap.
 */
export function RobotRing({ className, around = false, ...props }: { className?: string; around?: boolean; "data-icon"?: "inline-start" }) {
  const radius = around ? 7 : 6
  const stroke = around ? 1.5 : 2
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden data-testid="robot-ring" className={cn("animate-robot-ring", className)} {...props}>
      <circle cx="8" cy="8" r={radius} stroke="currentColor" strokeOpacity={0.25} strokeWidth={stroke} />
      <path d={`M8 ${8 - radius}a${radius} ${radius} 0 0 1 ${radius} ${radius}`} stroke="currentColor" strokeWidth={stroke} strokeLinecap="round" />
    </svg>
  )
}

/**
 * How far a running robot is: `progress` (0 to 1) of its usual duration, or, without history and past the usual time, an
 * indeterminate bar that runs across. The track is the primary tint, the fill primary, as the ring; `destructive` for a
 * robot that will fail.
 */
export function RobotProgressBar({ progress, failing = false, className }: { progress: number | null; failing?: boolean; className?: string }) {
  return (
    <ProgressLine
      progress={progress}
      label="Robot progress"
      tone={failing ? "destructive" : "primary"}
      className={className}
      data-testid="robot-progress"
      data-failing={failing || undefined}
    />
  )
}
