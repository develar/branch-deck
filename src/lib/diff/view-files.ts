import type { CommitFile } from "@/lib/bindings"
import type { DiffContent, FileDiff, TextDiff } from "@/lib/diff/file-diff-model"
import { languageOfPath } from "@/lib/diff/file-languages"
import type { FileMerge, MergeContent } from "@/lib/diff/file-merge-model"
import type { FileRows } from "@/lib/diff/rows"
import type { BlobTextEntry } from "@/stores/blob-texts"

/*
 * What `DiffView` shows of a file besides its rows (docs/design/diff-view.md, "One component for every view"): the
 * notice under its header, whether it starts collapsed, the grammar of its path.
 */

/** A file with more changed lines starts collapsed: its header is enough to know it is there */
export const COLLAPSE_CHANGED_LINES = 1000

/** Lock files and generated files start collapsed */
const GENERATED_FILE = /(^|\/)(pnpm-lock\.yaml|package-lock\.json|yarn\.lock|Cargo\.lock|Gemfile\.lock|poetry\.lock|go\.sum)$|\.(min\.js|min\.css|snap|map)$/

const megabytes = new Intl.NumberFormat("en", { maximumFractionDigits: 1 })

/** The Shiki language of a path; "text" for a file without a bundled grammar */
export function languageOf(path: string): string {
  return languageOfPath(path)
}

/**
 * Git's files of a subject with the line counts of the engine, the one source of the counts a diff shows (git numstat
 * counts the changes the policy ignores too). A file without its diff yet has no counts: no number turns into another.
 */
export function withEngineCounts(files: readonly CommitFile[], diffs: readonly FileDiff[] | undefined): CommitFile[] {
  const byPath = new Map(diffs?.map(diff => [diff.path, diff]))
  return files.map(file => {
    const content = byPath.get(file.path)?.content
    const additions = content?.kind === "text" ? content.additions : 0
    const deletions = content?.kind === "text" ? content.deletions : 0
    return additions === file.additions && deletions === file.deletions ? file : { ...file, additions, deletions }
  })
}

/** The line ends of a file's notice: "LF → CRLF", or that they are the only change */
export function lineEndsNotice(diff: TextDiff): string | null {
  const { lineEnds } = diff
  if (lineEnds === null) {
    return null
  }
  const names = { lf: "LF", crlf: "CRLF", cr: "CR", mixed: "mixed" } as const
  return lineEnds.only ? `Only line endings changed: ${names[lineEnds.old]} → ${names[lineEnds.new]}` : `Line endings: ${names[lineEnds.old]} → ${names[lineEnds.new]}`
}

/** Why a text of a file cannot show: null while it is read, or when it is a text */
export function blobNotice(entry: BlobTextEntry | undefined): string | null {
  if (entry === undefined || entry.kind === "loading") {
    return null
  }
  if (entry.kind === "error") {
    return `Cannot read the file: ${entry.message}`
  }
  switch (entry.blob.kind) {
    case "text":
      return null
    case "binary":
      return "Binary file"
    case "tooLarge":
      return `Too large to show: ${megabytes.format(entry.blob.size / 1024 / 1024)} MB`
    case "missing":
      return "The file is not in the repository"
  }
}

const shortCommit = (id: string | null) => (id === null ? "none" : id.slice(0, 8))

/** Why a file shows no lines, from its content; null for a text */
export function contentNotice(content: DiffContent): string | null {
  switch (content.kind) {
    case "text":
      return null
    case "binary":
      return "Binary file"
    case "tooLarge":
      return `Too large to show: ${megabytes.format(Math.max(content.oldBytes, content.newBytes) / 1024 / 1024)} MB`
    case "submodule":
      return `Submodule: ${shortCommit(content.oldCommit)} → ${shortCommit(content.newCommit)}`
    case "missing":
      return "Git cannot read the file: the clone is partial or shallow"
  }
}

/**
 * Why a file shows no rows, or how its line ends changed: under its header. Null while the reason is not known yet
 * (its diff or a text is being read) and for a file that shows rows with the same line ends.
 */
export function fileNotice(diff: FileDiff | undefined, rows: FileRows | null, texts: ReadonlyMap<string, BlobTextEntry>): string | null {
  if (diff === undefined) {
    return "No changed lines"
  }
  const { content } = diff
  if (content.kind !== "text") {
    return contentNotice(content)
  }
  for (const id of [diff.oldId, diff.newId]) {
    const notice = id === null ? null : blobNotice(texts.get(id))
    if (notice !== null) {
      return notice
    }
  }
  const lineEnds = lineEndsNotice(content)
  if (rows !== null && rows.rowCount === 0) {
    return lineEnds ?? "No changed lines"
  }
  return lineEnds
}

/** Why a conflicting file shows no lines, from its content; null for a text */
export function mergeContentNotice(content: MergeContent): string | null {
  switch (content.kind) {
    case "text":
      return null
    case "binary":
      return "Binary file"
    case "tooLarge":
      return `Too large to show: ${megabytes.format(Math.max(content.baseBytes, content.leftBytes, content.rightBytes) / 1024 / 1024)} MB`
    case "submodule":
      return `Submodule: ${shortCommit(content.leftCommit)} and ${shortCommit(content.rightCommit)}, from ${shortCommit(content.baseCommit)}`
    case "missing":
      return "Git cannot read the file: the clone is partial or shallow"
    case "oneSided":
      return content.deleted === "left" ? "The target deleted the file, the commit changed it" : "The commit deleted the file, the target changed it"
  }
}

/**
 * Why a conflicting file shows no rows, or what its conflicts are: under it. A conflict of git whose words resolve
 * says so; null while a text is read and for conflicts that do not resolve.
 */
export function mergeNotice(merge: FileMerge | undefined, rows: FileRows | null, texts: ReadonlyMap<string, BlobTextEntry>): string | null {
  if (merge === undefined) {
    return "No conflict"
  }
  const { content } = merge
  if (content.kind !== "text") {
    return mergeContentNotice(content)
  }
  for (const id of [merge.leftId, merge.baseId, merge.rightId]) {
    const notice = id === null ? null : blobNotice(texts.get(id))
    if (notice !== null) {
      return notice
    }
  }
  const { conflicts, unresolved } = content
  if (conflicts > 0 && unresolved === 0) {
    return conflicts === 1 ? "Git stops here; the conflict resolves by words" : "Git stops here; the conflicts resolve by words"
  }
  if (unresolved < conflicts) {
    return `${conflicts} conflicts, ${conflicts - unresolved} of them resolve by words`
  }
  return rows !== null && rows.rowCount === 0 ? "No changed lines" : null
}

/** The notice says why the file shows no rows: the header cannot expand it */
export function isEmptyNotice(diff: FileDiff | undefined, rows: FileRows | null): boolean {
  return diff === undefined || diff.content.kind !== "text" || (rows !== null && rows.rowCount === 0)
}

/** Lock and generated files, files over `COLLAPSE_CHANGED_LINES` and files without lines start collapsed */
export function startsCollapsed(file: CommitFile, diff: FileDiff | undefined): boolean {
  if (diff === undefined || diff.content.kind !== "text") {
    return true
  }
  return diff.content.additions + diff.content.deletions > COLLAPSE_CHANGED_LINES || GENERATED_FILE.test(file.path)
}

/** The text of a blob once it is read; null while it is read, or when it is not text */
export function blobText(texts: ReadonlyMap<string, BlobTextEntry>, id: string | null): string | null {
  if (id === null) {
    return ""
  }
  const entry = texts.get(id)
  return entry?.kind === "loaded" && entry.blob.kind === "text" ? entry.blob.text : null
}
