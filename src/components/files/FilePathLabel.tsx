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
  /**
   * The header of a file above its diff: the name `font-semibold`, whole while the directory gives way (from its start,
   * so its last folder shows as long as any of it does), the last folder in the text colour, since it is what tells
   * apart ten files of one name in a row; only a name wider than the label truncates
   */
  emphasis?: boolean
  /** The letter in the `size-4` slot of a folder icon, so that the names of a tree align */
  letterSlot?: "inline" | "icon"
  /**
   * The directory whole or not at all (a row of a tree): it shows only where it fits after the name, never cut to its
   * last letters ("…a"); the name truncates as the last resort
   */
  wholeDirectory?: boolean
  className?: string
}

/**
 * A changed file wherever it is listed: the status letter, the name in the colour of its status, where it came from, then
 * its directory muted, in the size around it. The name comes first, as IntelliJ lists a change; a long directory gives
 * way first and loses its start (`dir="rtl"`, the path itself kept left to right by `bdi`).
 */
export function FilePathLabel({ path, oldPath = null, status, directory = true, emphasis = false, letterSlot = "inline", wholeDirectory = false, className }: FilePathLabelProps) {
  const label = formatTreeFileLabel({ path, oldPath })
  const parent = directory ? path.slice(0, path.length - label.name.length).replace(/\/$/, "") : ""
  const statusClass = status === undefined ? undefined : getFileStatusTextClass(status)
  if (wholeDirectory && parent !== "") {
    return (
      // one line of a wrapping row: the directory that does not fit after the name wraps below it, out of sight
      <span className={cn("flex h-[1lh] min-w-0 flex-wrap items-baseline gap-x-1.5 overflow-hidden", className)}>
        <span className="flex max-w-full min-w-0 items-baseline gap-1.5">
          {status !== undefined && <StatusLetter status={status} letterSlot={letterSlot} />}
          <span className={cn("min-w-0 truncate", statusClass)}>{label.name}</span>
          {label.from !== null && <span className="min-w-0 shrink-[10] truncate text-muted-foreground">{`← ${label.from}`}</span>}
        </span>
        <span className="shrink-0 text-muted-foreground" data-testid="file-label-directory">
          {parent}
        </span>
      </span>
    )
  }
  return (
    <span className={cn("flex min-w-0 items-baseline gap-1.5", className)}>
      {status !== undefined && <StatusLetter status={status} letterSlot={letterSlot} />}
      <span
        className={cn(
          "min-w-0 truncate",
          statusClass,
          // the width of the label less the letter and its gap: a name longer than that truncates as the last resort
          emphasis && ["shrink-0 font-semibold", status === undefined ? "max-w-full" : letterSlot === "icon" ? "max-w-[calc(100%-1.375rem)]" : "max-w-[calc(100%-1.125rem)]"],
        )}
      >
        {label.name}
      </span>
      {label.from !== null && <span className="min-w-0 shrink-[10] truncate text-muted-foreground">{`← ${label.from}`}</span>}
      {parent !== "" && (
        <span className="min-w-0 shrink-[10] truncate text-muted-foreground" dir="rtl">
          <bdi dir="ltr">{emphasis ? <EmphasizedDirectory directory={parent} /> : parent}</bdi>
        </span>
      )}
    </span>
  )
}

/** A directory with its last folder in the text colour: `.claude/skills/` muted, `find-skills` not */
function EmphasizedDirectory({ directory }: { directory: string }) {
  const slash = directory.lastIndexOf("/")
  return (
    <>
      {directory.slice(0, slash + 1)}
      <span className="text-foreground">{directory.slice(slash + 1)}</span>
    </>
  )
}

/** The letter of a status before the name */
function StatusLetter({ status, letterSlot }: { status: FileStatus; letterSlot: "inline" | "icon" }) {
  return (
    // the status class first: a deleted file strikes its name through, never its letter
    <span aria-hidden className={cn("shrink-0 text-center font-mono text-xs", letterSlot === "icon" ? "w-4" : "w-3", getFileStatusTextClass(status), "no-underline")}>
      {getFileStatusLetter(status)}
    </span>
  )
}
