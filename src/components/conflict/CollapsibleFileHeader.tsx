import type { ReactNode } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { cn } from "cn"
import { FileContextMenu } from "@/components/files/FileContextMenu"
import { FILE_HEADER_CLASS_NAME, FileHeaderChevron, FileHeaderCopyButton } from "@/components/files/FileHeaderParts"
import { FilePathLabel } from "@/components/files/FilePathLabel"
import type { FileStatus } from "@/lib/bindings"

export interface CollapsibleFileHeaderProps {
  /** The path the header names and the copy button copies */
  path: string
  /** The path before a rename */
  oldPath?: string | null
  /** The status of a changed file (the uncommitted changes); a conflicting file has none */
  status?: FileStatus
  /** At the end of the header (e.g. the conflict count, the line counts); must not be interactive */
  actions?: ReactNode
}

/**
 * Accordion item header naming a file as every list of changed files does (`FilePathLabel`), in the parts of a review's
 * file header (`FileHeaderParts`): the chevron first, the name, Copy Path on hover, the counts at the end. The trigger
 * button holds the chevron and the name, but its `::after` covers the whole header, so a click anywhere toggles the
 * item; the copy button sits above it (no button nested in the trigger).
 */
export function CollapsibleFileHeader({ path, oldPath = null, status, actions }: CollapsibleFileHeaderProps) {
  return (
    <FileContextMenu
      target={() => ({ kind: "file", path })}
      render={<AccordionPrimitive.Header className={cn(FILE_HEADER_CLASS_NAME, "relative transition-colors hover:bg-muted/50")} />}
    >
      <AccordionPrimitive.Trigger
        aria-label={path}
        className="flex min-w-0 cursor-pointer items-center gap-2 text-left outline-none after:absolute after:inset-0 focus-visible:after:ring-3 focus-visible:after:ring-ring/30 focus-visible:after:ring-inset"
      >
        {/* where the chevron button of a review header is */}
        <span className="flex size-6 shrink-0 items-center justify-center text-muted-foreground">
          <FileHeaderChevron openClassName="group-data-open/file-header:rotate-90" />
        </span>
        <FilePathLabel path={path} oldPath={oldPath} status={status} emphasis />
      </AccordionPrimitive.Trigger>
      <FileHeaderCopyButton path={path} className="relative z-10 focus-within:opacity-100" />
      {/* never shrinks: a long path truncates, the counts stay on one line */}
      {actions !== undefined && <div className="ml-auto flex shrink-0 items-center gap-3 text-xs">{actions}</div>}
    </FileContextMenu>
  )
}
