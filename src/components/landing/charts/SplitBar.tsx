import { ChartTooltip } from "@/components/landing/charts/ChartHover"
import { useChartHover } from "@/hooks/useChartHover"

export interface SplitPart {
  readonly label: string
  readonly value: number
  /** The text of the value, e.g. "6 h" */
  readonly text: string
  /** The fill, a token class (`bg-success`) */
  readonly fill: string
}

/**
 * One bar split into its parts by share, a 2px gap between them, and the legend under it with each part's value: the
 * parts are named by their label, never by their colour alone
 */
export function SplitBar({ parts, label }: { parts: readonly SplitPart[]; label: string }) {
  const { hover, markProps } = useChartHover()
  const total = parts.reduce((sum, part) => sum + part.value, 0)
  const shown = parts.filter(part => part.value > 0)
  // the middle of each part, where its tooltip points
  const centers = shown.map((part, index) => (shown.slice(0, index).reduce((sum, earlier) => sum + earlier.value, 0) + part.value / 2) / total)
  const hovered = hover === null ? undefined : shown[hover.index]
  return (
    <figure className="flex flex-col gap-2">
      <div className="relative">
        <div role="img" aria-label={`${label}: ${parts.map(part => `${part.label} ${part.text}`).join(", ")}`} className="flex h-3 gap-0.5" data-testid="split-bar">
          {total === 0 && <div className="h-full w-full rounded-sm bg-muted" />}
          {shown.map((part, index) => (
            <div
              key={part.label}
              tabIndex={0}
              aria-label={`${part.label}: ${part.text}`}
              className={`h-full min-w-1 rounded-sm outline-none first:rounded-l-md last:rounded-r-md focus-visible:ring-2 focus-visible:ring-ring ${part.fill}`}
              style={{ flexGrow: part.value, flexBasis: 0, opacity: hover === null || hover.index === index ? 1 : 0.5 }}
              {...markProps(index, centers[index] ?? 0)}
            />
          ))}
        </div>
        <ChartTooltip hover={hover}>{hovered !== undefined && `${hovered.label} · ${hovered.text} · ${Math.round((hovered.value / total) * 100)}%`}</ChartTooltip>
      </div>
      <figcaption>
        <ul className="flex flex-wrap gap-x-4 gap-y-1 text-xs">
          {parts.map(part => (
            <li key={part.label} className="flex items-center gap-1.5">
              <span aria-hidden className={`size-2 rounded-sm ${part.fill}`} />
              <span className="text-muted-foreground">{part.label}</span>
              <span className="font-medium text-foreground tabular-nums">{part.text}</span>
            </li>
          ))}
        </ul>
      </figcaption>
    </figure>
  )
}
