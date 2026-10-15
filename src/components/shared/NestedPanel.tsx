import type { ComponentProps } from "react"
import { cn } from "cn"
import { Card } from "@/components/ui/card"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import type { SectionCardHeaderProps } from "@/components/shared/SectionCardHeader"

export interface NestedPanelProps extends ComponentProps<"div"> {
  /** The header row, above a `border-border` rule */
  header?: SectionCardHeaderProps
}

/**
 * A panel inside an island (the expanded row of a branch): `rounded-lg border border-border bg-card`, without the ring
 * and the shadow of a `Card`, so it stays visibly less round and flatter than the island around it.
 */
export function NestedPanel({ header, className, children, ...props }: NestedPanelProps) {
  return (
    <div data-slot="nested-panel" className={cn("flex flex-col overflow-hidden rounded-lg border border-border bg-card", className)} {...props}>
      {header && <PanelHeader {...header} />}
      {children}
    </div>
  )
}

export interface SectionPanelProps extends NestedPanelProps {
  /** A top-level block of its own (a sub-window page): a `Card` island instead of a nested panel */
  island: boolean
}

/** A section shown both nested in the expanded row and as an island of a sub-window page. */
export function SectionPanel({ island, header, className, children, ...props }: SectionPanelProps) {
  if (!island) {
    return <NestedPanel header={header} className={className} {...props}>{children}</NestedPanel>
  }
  return (
    <Card className={cn("gap-0 py-0", className)} {...props}>
      {header && <PanelHeader {...header} />}
      {children}
    </Card>
  )
}

function PanelHeader(props: SectionCardHeaderProps) {
  return (
    <div className="border-b border-border px-4 py-2.5 sm:px-6">
      <SectionCardHeader {...props} />
    </div>
  )
}
