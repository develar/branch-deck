import type { ReactNode } from "react"
import { InfoIcon } from "lucide-react"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"

export interface HelpPopoverProps {
  /** Names the help for screen readers, e.g. "About unassigned commits" */
  "label": string
  /** On the trigger button */
  "data-testid"?: string
  /** `HelpSection`s, or paragraphs */
  "children": ReactNode
}

/** An `InfoIcon` button that explains a section on hover or click. Never put it inside another button. */
export function HelpPopover({ label, "data-testid": testId, children }: HelpPopoverProps) {
  return (
    <Popover>
      <PopoverTrigger
        openOnHover
        aria-label={label}
        data-testid={testId}
        className="inline-flex cursor-help rounded-sm text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30"
      >
        <InfoIcon className="size-3.5" />
      </PopoverTrigger>
      <HelpPopoverContent>{children}</HelpPopoverContent>
    </Popover>
  )
}

/** The popup of a help popover, for a trigger other than the info icon (the AI toggle) */
export function HelpPopoverContent({ children }: { children: ReactNode }) {
  return <PopoverContent className="w-80 gap-3 p-3 text-xs">{children}</PopoverContent>
}

/** A muted `font-medium` head (sentence case) over its text */
export function HelpSection({ title, children }: { title: ReactNode, children: ReactNode }) {
  return (
    <section className="flex flex-col gap-1.5">
      <h3 className="font-medium text-muted-foreground">{title}</h3>
      {children}
    </section>
  )
}

export function HelpList({ children }: { children: ReactNode }) {
  return <ul className="flex list-disc flex-col gap-1 pl-4 marker:text-muted-foreground">{children}</ul>
}
