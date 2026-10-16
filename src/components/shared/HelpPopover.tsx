import type { ReactNode } from "react"
import { cn } from "cn"
import { InfoIcon } from "lucide-react"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { Popover, PopoverContent, PopoverDescription, PopoverHeader, PopoverSectionTitle, PopoverTitle, PopoverTrigger } from "@/components/ui/popover"

export interface HelpPopoverProps {
  /** The name of what it explains, as its title reads ("Archived Branches"): the popup's title, the trigger "About …" */
  title: string
  /** On the trigger button */
  "data-testid"?: string
  /** 1–3 short paragraphs, `HelpSection`s for a second kind of content */
  children: ReactNode
}

/**
 * An `InfoIcon` button right after the title it explains (`SectionCardHeader` and `CollapsibleCard` take it as `help`),
 * opening on hover or click. Never put it inside another button.
 */
export function HelpPopover({ title, "data-testid": testId, children }: HelpPopoverProps) {
  return (
    <Popover>
      <PopoverTrigger
        openOnHover
        aria-label={`About ${title}`}
        data-testid={testId}
        className="inline-flex shrink-0 cursor-help rounded-sm text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/30"
      >
        <InfoIcon className="size-3.5" />
      </PopoverTrigger>
      <HelpPopoverContent title={title}>{children}</HelpPopoverContent>
    </Popover>
  )
}

export interface HelpPopoverContentProps {
  title: ReactNode
  /** One muted line under the title */
  description?: ReactNode
  /** `destructive`: the title of a failure (the AI toggle after an error) */
  tone?: "destructive"
  children: ReactNode
}

/** The popup of a help popover: a title, an optional description, the body; for a trigger of its own (the AI toggle) */
export function HelpPopoverContent({ title, description, tone, children }: HelpPopoverContentProps) {
  return (
    <PopoverContent align="start" className="w-80 gap-3 p-3 text-xs">
      <PopoverHeader className="gap-0.5">
        <PopoverTitle className={cn("text-sm", tone === "destructive" && "text-destructive")}>{title}</PopoverTitle>
        {description !== undefined && <PopoverDescription className="text-xs break-words">{description}</PopoverDescription>}
      </PopoverHeader>
      {children}
    </PopoverContent>
  )
}

/** A section of a help popover under a `PopoverSectionTitle` (sentence case) */
export function HelpSection({ title, children }: { title: ReactNode; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-1.5">
      <PopoverSectionTitle>{title}</PopoverSectionTitle>
      {children}
    </section>
  )
}

export function HelpList({ children }: { children: ReactNode }) {
  return <ul className="flex list-disc flex-col gap-1 pl-4 marker:text-muted-foreground">{children}</ul>
}

/** Keys and what they do, a `HelpKey` per row: the keys beside the label, as the details `dl` of a status popover */
export function HelpKeys({ children }: { children: ReactNode }) {
  return <dl className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-1">{children}</dl>
}

export interface HelpKeyProps {
  /** `Kbd` labels, from `@/lib/platform` for a modifier (`modifierKey`, `shiftKey`) */
  keys?: readonly string[]
  /** With a click: "⌘ Click"; a click is no key, so it is text */
  click?: boolean
  children: ReactNode
}

export function HelpKey({ keys = [], click = false, children }: HelpKeyProps) {
  return (
    <>
      <dt className="flex items-center gap-1 text-muted-foreground">
        {keys.length > 0 && (
          <KbdGroup>
            {keys.map(key => (
              <Kbd key={key}>{key}</Kbd>
            ))}
          </KbdGroup>
        )}
        {click && <span>Click</span>}
      </dt>
      <dd>{children}</dd>
    </>
  )
}
