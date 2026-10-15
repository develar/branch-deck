import type { CommitFile, CommitFiles, FileStatus } from "@/lib/bindings"
import type { UIColor } from "@/lib/ui-types"

/** What the meta line of a commit says about its files: "3 files · +42 −7" */
export interface CommitFileStats {
  count: number
  additions: number
  deletions: number
}

export function getCommitFileStats(files: CommitFiles): CommitFileStats {
  return { count: files.files.length, additions: files.additions, deletions: files.deletions }
}

/** The paths the commits changed (a renamed file by its new path), each once, in the order of the commits */
export function getChangedPaths(list: readonly CommitFiles[]): string[] {
  return [...new Set(list.flatMap(files => files.files.map(file => file.path)))]
}

/** The badge colour of a file status, the same in the uncommitted changes and in a commit */
export function getFileStatusColor(status: string): UIColor {
  switch (status) {
    case "added":
      return "success"
    case "modified":
      return "info"
    case "deleted":
      return "error"
    case "renamed":
    case "copied":
    case "typeChanged":
      return "neutral"
    default:
      return "info"
  }
}

/** The letter of a file status in the file tree, as git prints it in `--name-status` */
export function getFileStatusLetter(status: FileStatus): string {
  switch (status) {
    case "added":
      return "A"
    case "deleted":
      return "D"
    case "renamed":
      return "R"
    case "copied":
      return "C"
    case "typeChanged":
      return "T"
    default:
      return "M"
  }
}

/** The colour of a file name in the file tree, as IntelliJ colours a change: added green, deleted struck through, the rest blue */
export function getFileStatusTextClass(status: FileStatus): string {
  switch (status) {
    case "added":
      return "text-success"
    case "deleted":
      return "text-muted-foreground line-through"
    default:
      return "text-primary"
  }
}

/** The badge text of a file status */
export function formatFileStatus(status: FileStatus): string {
  return status === "typeChanged" ? "type changed" : status
}

/** The path a file header shows: `old → new` for a rename or a copy */
export function formatCommitFilePath(file: CommitFile): string {
  return file.oldPath === null ? file.path : `${file.oldPath} → ${file.path}`
}
