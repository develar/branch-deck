import { cn } from "cn"
import type { FileStatus } from "@/lib/bindings"
import { getFileStatusLetter, getFileStatusTextClass } from "@/lib/commit-files"
import { formatTreeFileLabel } from "@/lib/file-tree"

export interface FilePathLabelProps {
  path: string
  /** The path before a rename or a copy: "← Old.kt" after the name */
  oldPath?: string | null
  /** The letter before the name and the colour of the name; without it the name is in the text colour */
  status?: FileStatus
  /** The directory after the name; a tree row names it in the rows above */
  directory?: boolean
  /** `font-medium` name: the header of a file above its diff */
  emphasis?: boolean
  /** The letter in the `size-4` slot of a folder icon, so that the names of a tree align */
  letterSlot?: "inline" | "icon"
  className?: string
}

/**
 * A changed file wherever it is listed: the status letter, the name in the colour of its status, where it came from, then
 * its directory muted, in the size around it. The name comes first, as IntelliJ lists a change; a long directory gives
 * way first and loses its start (`dir="rtl"`, the path itself kept left to right by `bdi`).
 */
export function FilePathLabel({ path, oldPath = null, status, directory = true, emphasis = false, letterSlot = "inline", className }: FilePathLabelProps) {
  const label = formatTreeFileLabel({ path, oldPath })
  const parent = directory ? path.slice(0, path.length - label.name.length).replace(/\/$/, "") : ""
  const statusClass = status === undefined ? undefined : getFileStatusTextClass(status)
  return (
    <span className={cn("flex min-w-0 items-baseline gap-1.5", className)}>
      {status !== undefined && (
        // the status class first: a deleted file strikes its name through, never its letter
        <span aria-hidden className={cn("shrink-0 text-center font-mono text-xs", letterSlot === "icon" ? "w-4" : "w-3", statusClass, "no-underline")}>
          {getFileStatusLetter(status)}
        </span>
      )}
      <span className={cn("min-w-0 truncate", statusClass, emphasis && "font-medium")}>{label.name}</span>
      {label.from !== null && <span className="min-w-0 shrink-[10] truncate text-muted-foreground">{`← ${label.from}`}</span>}
      {parent !== "" && (
        <span className="min-w-0 shrink-[10] truncate text-muted-foreground" dir="rtl">
          <bdi dir="ltr">{parent}</bdi>
        </span>
      )}
    </span>
  )
}
