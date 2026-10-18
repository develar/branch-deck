import type { ReactNode } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { ChevronDownIcon } from "lucide-react"
import { FileContextMenu } from "@/components/files/FileContextMenu"
import { FilePathLabel } from "@/components/files/FilePathLabel"
import { CopyButton } from "@/components/shared/CopyButton"
import type { FileStatus } from "@/lib/bindings"

export interface CollapsibleFileHeaderProps {
  /** The path the header names and the copy button copies */
  path: string
  /** The path before a rename */
  oldPath?: string | null
  /** The status of a changed file (the uncommitted changes); a conflicting file has none */
  status?: FileStatus
  /** Rendered before the chevron (e.g. the conflict count); must not be interactive */
  actions?: ReactNode
}

/**
 * Accordion item header naming a file as every list of changed files does (`FilePathLabel`). The trigger button holds only the name, but its `::after` covers the
 * whole header, so a click anywhere toggles the item; the copy button sits above it (no button nested in the trigger).
 */
export function CollapsibleFileHeader({ path, oldPath = null, status, actions }: CollapsibleFileHeaderProps) {
  return (
    <FileContextMenu
      target={() => ({ kind: "file", path })}
      render={<AccordionPrimitive.Header className="group/file-header relative flex items-center gap-1 px-3 py-2 transition-colors hover:bg-muted/50" />}
    >
      <AccordionPrimitive.Trigger
        aria-label={path}
        className="flex min-w-0 cursor-pointer text-left text-sm outline-none after:absolute after:inset-0 focus-visible:after:ring-3 focus-visible:after:ring-ring/30 focus-visible:after:ring-inset"
      >
        <FilePathLabel path={path} oldPath={oldPath} status={status} />
      </AccordionPrimitive.Trigger>
      <CopyButton
        text={() => path}
        tooltip="Copy file name to clipboard"
        // hidden without delay, shown on header hover after a delay
        className="relative z-10 duration-200 ease-out group-hover/file-header:opacity-100 group-hover/file-header:delay-300 focus-within:opacity-100"
      />
      {/* never shrinks: a long path truncates, the counts stay on one line */}
      <div className="ml-auto flex shrink-0 items-center gap-1">
        {actions}
        <ChevronDownIcon className="size-4 text-muted-foreground transition-transform duration-200 group-data-open/file-header:rotate-180" />
      </div>
    </FileContextMenu>
  )
}
