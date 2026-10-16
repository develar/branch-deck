import { cn } from "cn"

/**
 * The rotating ring of a running Patronus robot: a track and an arc, slower than a spinner (a robot runs for minutes).
 * Sized by its `Button` (`data-icon`) or by `className`; the colour is the text colour. `around` is the ring around the
 * node of the stack graph: wider and thinner, so the node keeps its gap.
 */
export function RobotRing({ className, around = false, ...props }: { "className"?: string, "around"?: boolean, "data-icon"?: "inline-start" }) {
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
export function RobotProgressBar({ progress, failing = false, className }: { progress: number | null, failing?: boolean, className?: string }) {
  return (
    <div
      role="progressbar"
      aria-label="Robot progress"
      aria-valuenow={progress === null ? undefined : Math.round(progress * 100)}
      data-testid="robot-progress"
      data-indeterminate={progress === null || undefined}
      data-failing={failing || undefined}
      className={cn("relative overflow-hidden", failing ? "bg-destructive/15" : "bg-primary/15", className)}
    >
      {progress === null
        ? <div className={cn("absolute inset-y-0 w-1/3 animate-carousel", failing ? "bg-destructive" : "bg-primary")} />
        : <div className={cn("absolute inset-y-0 left-0 transition-[width] duration-1000", failing ? "bg-destructive" : "bg-primary")} style={{ width: `${Math.min(progress, 1) * 100}%` }} />}
    </div>
  )
}
