import { useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { Card } from "@/components/ui/card"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"

export interface CollapsibleCardProps {
  "header": ReactNode
  /** Controls next to the header, outside its toggle button (a `HelpPopover`) */
  "actions"?: ReactNode
  "children": ReactNode
  "defaultOpen"?: boolean
  /** Controlled: the open state, kept by the caller (e.g. per repository); `defaultOpen` is ignored then */
  "open"?: boolean
  "onOpenChange"?: (open: boolean) => void
  /** Of the card: its entrance animation */
  "className"?: string
  "data-testid"?: string
  /** The root of the stack of a stack island (`StackIslandCard`), which the keys find its header by */
  "data-stack-root"?: string
}

/**
 * `Card` whose header toggles the body. The body is rendered only while open.
 * `data-state="open|closed"` is set explicitly on the root and the body (Base UI emits `data-open`/`data-closed`).
 * Inside the header, `group-data-[state=open]:` targets the open state.
 */
export function CollapsibleCard({ header, actions, children, defaultOpen = false, open: controlledOpen, onOpenChange, className, "data-testid": testId, "data-stack-root": stackRoot }: CollapsibleCardProps) {
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
      render={<Card className={cn("gap-0 py-0", open && "divide-y divide-border", className)} />}
    >
      <div className="flex items-center gap-3 px-4 py-2.5 sm:px-6">
        <CollapsibleTrigger data-state={state} className="group min-w-0 flex-1 cursor-pointer rounded-lg text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/30">
          {header}
        </CollapsibleTrigger>
        {actions}
      </div>

      {open && (
        <CollapsibleContent data-state={state} className="animate-in overflow-x-auto duration-150 fade-in-0">
          {children}
        </CollapsibleContent>
      )}
    </Collapsible>
  )
}
