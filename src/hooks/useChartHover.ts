import { useState } from "react"

/** The mark under the pointer or the focus, by its index, and where its tooltip goes (fractions of the plot) */
export interface ChartHover {
  readonly index: number
  /** 0 to 1 from the left of the plot */
  readonly x: number
}

/** The hovered mark of a chart; `markProps` gives each mark its pointer and focus handlers */
export function useChartHover() {
  const [hover, setHover] = useState<ChartHover | null>(null)
  const markProps = (index: number, x: number) => ({
    onPointerEnter: () => setHover({ index, x }),
    onPointerLeave: () => setHover(null),
    onFocus: () => setHover({ index, x }),
    onBlur: () => setHover(null),
  })
  return { hover, markProps }
}
