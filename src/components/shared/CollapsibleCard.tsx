import { useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { Card } from "@/components/ui/card"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"

export interface CollapsibleCardProps {
  /** Inside the toggle, after the chevron: the title, its count and note; no chevron of its own */
  header: ReactNode
  /** A `HelpPopover`, right after the header, outside the toggle button */
  help?: ReactNode
  /** Controls at the end of the header row, outside the toggle button */
  actions?: ReactNode
  children: ReactNode
  defaultOpen?: boolean
  /** Controlled: the open state, kept by the caller (e.g. per repository); `defaultOpen` is ignored then */
  open?: boolean
  onOpenChange?: (open: boolean) => void
  /** Of the card: its entrance animation */
  className?: string
  "data-testid"?: string
  /** The root of the stack of a stack island (`StackIslandCard`), which the keys find its header by */
  "data-stack-root"?: string
}

/**
 * `Card` whose header toggles the body. The body is rendered only while open. The header
 * row is one toggle button, the leading chevron and the header in it; its `::after` covers the row below the help and
 * the actions, so the whole row toggles while they stay clickable, with one tab stop.
 * `data-state="open|closed"` is set explicitly on the root and the body (Base UI emits `data-open`/`data-closed`).
 * Inside the header, `group-data-[state=open]:` targets the open state.
 */
export function CollapsibleCard({
  header,
  help,
  actions,
  children,
  defaultOpen = false,
  open: controlledOpen,
  onOpenChange,
  className,
  "data-testid": testId,
  "data-stack-root": stackRoot,
}: CollapsibleCardProps) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen)
  const open = controlledOpen ?? uncontrolledOpen
  const state = open ? "open" : "closed"

  return (
    <Collapsible
      open={open}
      onOpenChange={onOpenChange ?? setUncontrolledOpen}
      data-state={state}
      data-testid={testId}
      data-stack-root={stackRoot}
      // a divider between the header and the body; collapsed, the header is alone
      render={<Card className={cn("gap-0 py-0", open && "divide-y divide-border", className)} />}
    >
      <div className="relative isolate flex items-center gap-3 px-4 py-2.5 sm:px-6">
        <CollapsibleTrigger
          data-state={state}
          className="group flex min-w-0 cursor-pointer items-start gap-2 text-left outline-none after:absolute after:inset-0 after:-z-10 after:rounded-lg focus-visible:after:ring-3 focus-visible:after:ring-ring/30 focus-visible:after:ring-inset"
        >
          <span className="flex h-5 shrink-0 items-center">
            <ChevronRightIcon aria-hidden className="size-4 text-muted-foreground transition-transform duration-150 group-data-[state=open]:rotate-90" />
          </span>
          {header}
        </CollapsibleTrigger>
        {help}
        {actions !== undefined && <div className="ml-auto flex shrink-0 items-center gap-3">{actions}</div>}
      </div>

      {open && (
        <CollapsibleContent data-state={state} className="animate-in overflow-x-auto duration-150 fade-in-0">
          {children}
        </CollapsibleContent>
      )}
    </Collapsible>
  )
}
