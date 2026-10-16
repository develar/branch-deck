import type { ReactNode } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { ChevronDownIcon } from "lucide-react"
import { CopyButton } from "@/components/shared/CopyButton"

export interface CollapsibleFileHeaderProps {
  /** The path the copy button copies */
  fileName: string
  /** The header text; `fileName` by default (`old → new` for a rename) */
  label?: string
  /** Rendered before the chevron (e.g. a status badge); must not be interactive */
  actions?: ReactNode
}

/**
 * Accordion item header showing a file name. The trigger button holds only the file name, but its `::after` covers the
 * whole header, so a click anywhere toggles the item; the copy button sits above it (no button nested in the trigger).
 */
export function CollapsibleFileHeader({ fileName, label = fileName, actions }: CollapsibleFileHeaderProps) {
  return (
    <AccordionPrimitive.Header className="group/file-header relative flex items-center gap-1 px-3 py-2 transition-colors hover:bg-muted/50">
      <AccordionPrimitive.Trigger className="min-w-0 cursor-pointer truncate text-left font-mono text-xs text-muted-foreground outline-none after:absolute after:inset-0 focus-visible:after:ring-3 focus-visible:after:ring-ring/30 focus-visible:after:ring-inset">
        {label}
      </AccordionPrimitive.Trigger>
      <CopyButton
        text={() => fileName}
        tooltip="Copy file name to clipboard"
        // hidden without delay, shown on header hover after a delay
        className="relative z-10 duration-200 ease-out group-hover/file-header:opacity-100 group-hover/file-header:delay-300 focus-within:opacity-100"
      />
      {/* never shrinks: a long path truncates, the counts stay on one line */}
      <div className="ml-auto flex shrink-0 items-center gap-1">
        {actions}
        <ChevronDownIcon className="size-4 text-muted-foreground transition-transform duration-200 group-data-open/file-header:rotate-180" />
      </div>
    </AccordionPrimitive.Header>
  )
}
