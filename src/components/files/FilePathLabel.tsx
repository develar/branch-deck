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
  /**
   * The directory after the name; a tree row names it in the rows above. A string names it so: a file of a group of a
   * module of a review by its directory below the root of the module
   */
  directory?: boolean | string
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
 * its directory muted, in the size around it, right after the name. The name comes first, as IntelliJ lists a change: it
 * stays whole while the directory gives way, and only a name wider than the label truncates; nothing of it ever paints
 * over the directory. The directory loses its start (`dir="rtl"`, the path itself kept left to right by `bdi`) down to
 * its last folder, and a directory whose last folder does not fit after the name wraps out of sight, never cut to its
 * last letters ("…n"); a row of a tree shows it whole or not at all (`wholeDirectory`).
 */
export function FilePathLabel({ path, oldPath = null, status, directory = true, emphasis = false, letterSlot = "inline", wholeDirectory = false, className }: FilePathLabelProps) {
  const label = formatTreeFileLabel({ path, oldPath })
  const parent = typeof directory === "string" ? directory : directory ? path.slice(0, path.length - label.name.length).replace(/\/$/, "") : ""
  const statusClass = status === undefined ? undefined : getFileStatusTextClass(status)
  return (
    // one line of a wrapping row: a directory that does not fit after the name wraps below it, out of sight
    <span className={cn("flex h-[1lh] min-w-0 flex-wrap items-baseline gap-x-1.5 overflow-hidden", className)}>
      <span className="flex max-w-full min-w-0 items-baseline gap-1.5">
        {status !== undefined && <StatusLetter status={status} letterSlot={letterSlot} />}
        {emphasis ? <span className={cn("min-w-0 truncate font-semibold", statusClass)}>{label.name}</span> : <MidTruncatedName name={label.name} className={statusClass} />}
        {label.from !== null && <span className="min-w-0 shrink-[10] truncate text-muted-foreground">{`← ${label.from}`}</span>}
      </span>
      {parent !== "" &&
        (wholeDirectory ? (
          <span className="shrink-0 text-muted-foreground" data-testid="file-label-directory">
            {parent}
          </span>
        ) : (
          <span
            className="max-w-max min-w-0 grow truncate text-muted-foreground"
            // on the line of the name only with room for "…" and its last folder, then as much more as fits
            style={{ flexBasis: `${parent.length - parent.lastIndexOf("/") + 1}ch` }}
            dir="rtl"
            data-testid="file-label-directory"
          >
            <bdi dir="ltr">{emphasis ? <EmphasizedDirectory directory={parent} /> : parent}</bdi>
          </span>
        ))}
    </span>
  )
}

/**
 * A filename cut in the middle when needed: the start truncates, the extension and its suffix never do. As wide as the
 * name, never wider than its box: a suffix wider than the box is clipped at its edge, never drawn over what follows
 */
function MidTruncatedName({ name, className }: { name: string; className?: string }) {
  const dot = name.lastIndexOf(".")
  const suffixStart = dot > 0 ? Math.max(0, dot - 6) : Math.max(0, name.length - 6)
  return (
    <span className={cn("flex min-w-0 items-baseline overflow-hidden", className)} data-testid="file-path-label-name">
      <span className="min-w-0 truncate" data-testid="file-path-label-name-start">
        {name.slice(0, suffixStart)}
      </span>
      <span className="shrink-0" data-testid="file-path-label-name-suffix">
        {name.slice(suffixStart)}
      </span>
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
