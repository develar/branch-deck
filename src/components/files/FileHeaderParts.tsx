import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { CopyButton } from "@/components/shared/CopyButton"

// The parts of a file header, the same in a review and in the conflict and missing-commits accordions: the chevron
// first, the name, Copy Path on hover; the header is a `group/file-header`

/** The header of a file: `h-9`, the parts `gap-2` apart; the classes of its own container are the caller's */
export const FILE_HEADER_CLASS_NAME = "group/file-header flex h-9 items-center gap-2 px-2 text-sm"

/** The chevron of a file header, turned down while the file is open (`open`, or `openClassName` for a CSS state) */
export function FileHeaderChevron({ open, openClassName }: { open?: boolean; openClassName?: string }) {
  return <ChevronRightIcon className={cn("size-3 shrink-0 transition-transform duration-150", open === true && "rotate-90", openClassName)} />
}

/** Copy Path of a file header, shown on the hover of the header after a delay */
export function FileHeaderCopyButton({ path, className }: { path: string; className?: string }) {
  const name = path.slice(path.lastIndexOf("/") + 1)
  return (
    <CopyButton
      text={() => path}
      tooltip="Copy path"
      label={`Copy path of ${name}`}
      className={cn("duration-200 ease-out group-hover/file-header:opacity-100 group-hover/file-header:delay-300", className)}
    />
  )
}
