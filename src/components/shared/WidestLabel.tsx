import type { ReactNode } from "react"

/**
 * The label of a control that changes in place ("Copied" of a Copy button, "Updating 62%" of Update) in the width of the
 * widest it shows (ui-theme.md, "Live values hold their own width"): `widest` as an invisible pseudo-element in the
 * same cell of a grid, which holds the width and is no text of the control. A narrower "Copied" moved what came after the
 * button and shrank the popover or the footer it was in from under the pointer that clicked
 */
export function WidestLabel({ widest, children }: { widest: string; children: ReactNode }) {
  return (
    <span className="grid before:invisible before:col-start-1 before:row-start-1 before:content-[attr(data-widest)]" data-widest={widest}>
      <span className="col-start-1 row-start-1">{children}</span>
    </span>
  )
}
