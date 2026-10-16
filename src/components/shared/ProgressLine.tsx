import { cn } from "cn"

interface ProgressLineProps extends Omit<React.ComponentProps<"div">, "role" | "children"> {
  /** 0 to 1; null runs an indeterminate bar across */
  progress: number | null
  /** The accessible name, e.g. "Robot progress" */
  label: string
  /** What the screen reader says for the value, e.g. "Checking the update, 62%"; the percent alone without it */
  valueText?: string
  /** `destructive` for a run that will fail */
  tone?: "primary" | "destructive"
  /** How long the fill moves to a new value: `slow` (1 s) for a robot's minutes, `fast` for a tick of a quarter second */
  speed?: "slow" | "fast"
}

/**
 * A thin progress bar: the track is a tint of the tone, the fill the tone. The fill scales from the left (`scaleX`, no
 * layout per tick), clamped to the track.
 */
export function ProgressLine({ progress, label, valueText, tone = "primary", speed = "slow", className, ...props }: ProgressLineProps) {
  const fill = tone === "destructive" ? "bg-destructive" : "bg-primary"
  return (
    <div
      {...props}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={progress === null ? undefined : Math.round(Math.min(progress, 1) * 100)}
      aria-valuetext={valueText}
      data-indeterminate={progress === null || undefined}
      className={cn("relative overflow-hidden", tone === "destructive" ? "bg-destructive/15" : "bg-primary/15", className)}
    >
      {progress === null ? (
        <div className={cn("absolute inset-y-0 w-1/3 animate-carousel", fill)} />
      ) : (
        <div
          className={cn("absolute inset-0 origin-left transition-transform", speed === "slow" ? "duration-1000" : "duration-300 ease-linear", fill)}
          style={{ transform: `scaleX(${Math.min(Math.max(progress, 0), 1)})` }}
        />
      )}
    </div>
  )
}
