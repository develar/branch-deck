import { ChartTooltip } from "@/components/landing/charts/ChartHover"
import { useChartHover } from "@/hooks/useChartHover"
import type { StartHour } from "@/lib/bindings"
import { formatShortDuration } from "@/lib/robot-eta"

/** "10:00" */
function formatHour(hour: number): string {
  return `${String(hour).padStart(2, "0")}:00`
}

/**
 * The 24 workday hours: a bar per hour of everyone else's median time to merge from that start (shorter is faster),
 * the fastest fifth (`StartHour.level` 0) in success, the user's median as a dot on it; an hour without enough robots
 * stays empty
 */
export function HourProfile({ hours }: { hours: readonly StartHour[] }) {
  const { hover, markProps } = useChartHover()
  const max = Math.max(1, ...hours.flatMap(hour => [hour.everyone?.median ?? 0, hour.mine?.median ?? 0]))
  const hovered = hover === null ? undefined : hours[hover.index]
  const describe = (hour: StartHour) =>
    [
      formatHour(hour.hour),
      hour.everyone === null ? "too few robots" : `everyone ${formatShortDuration(hour.everyone.median)} (${hour.everyone.count})${hour.level === 0 ? ", among the fastest" : ""}`,
      ...(hour.mine === null ? [] : [`you ${formatShortDuration(hour.mine.median)} (${hour.mine.count})`]),
    ].join(" · ")
  return (
    <figure className="flex flex-col gap-1">
      <div className="relative">
        <div
          role="img"
          aria-label={`Median time to merge by start hour: ${hours
            .filter(hour => hour.everyone !== null)
            .map(describe)
            .join("; ")}`}
          className="flex h-24 items-end gap-0.5 border-b border-border"
          data-testid="hour-profile"
        >
          {hours.map((hour, index) => (
            <div
              key={hour.hour}
              tabIndex={hour.everyone === null && hour.mine === null ? -1 : 0}
              aria-label={describe(hour)}
              className="relative flex h-full flex-1 items-end outline-none focus-visible:ring-2 focus-visible:ring-ring"
              data-testid={`hour-${hour.hour}`}
              {...markProps(index, (index + 0.5) / hours.length)}
            >
              {hour.everyone !== null && (
                <div
                  className={hour.level === 0 ? "w-full rounded-t-sm bg-success" : "w-full rounded-t-sm bg-primary"}
                  style={{ height: `${(hour.everyone.median / max) * 100}%`, opacity: hover !== null && hover.index !== index ? 0.5 : 1 }}
                />
              )}
              {hour.mine !== null && (
                <span
                  aria-hidden
                  className="absolute left-1/2 size-2 -translate-x-1/2 translate-y-1/2 rounded-full bg-foreground ring-2 ring-card"
                  style={{ bottom: `${(hour.mine.median / max) * 100}%` }}
                />
              )}
            </div>
          ))}
        </div>
        <ChartTooltip hover={hover}>{hovered !== undefined && describe(hovered)}</ChartTooltip>
      </div>
      <figcaption className="flex flex-col gap-1 text-xs text-muted-foreground">
        <div className="flex justify-between">
          {[0, 6, 12, 18, 23].map(hour => (
            <span key={hour}>{formatHour(hour)}</span>
          ))}
        </div>
        <ul className="flex gap-4">
          <li className="flex items-center gap-1.5">
            <span aria-hidden className="size-2 rounded-sm bg-primary" />
            Everyone else, median
          </li>
          <li className="flex items-center gap-1.5">
            <span aria-hidden className="size-2 rounded-sm bg-success" />
            The fastest hours
          </li>
          <li className="flex items-center gap-1.5">
            <span aria-hidden className="size-2 rounded-full bg-foreground" />
            You
          </li>
        </ul>
      </figcaption>
    </figure>
  )
}
