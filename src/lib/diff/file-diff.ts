import { processFile } from "@pierre/diffs"
import type { FileDiffMetadata } from "@pierre/diffs"
import type { FileDiff } from "@/lib/bindings"

export interface ToFileDiffMetadataOptions {
  /** Keys the highlighted result in the worker cache; must change when the diff changes */
  cacheKey?: string
  /**
   * The file contents agree with the hunks (they come from the same two versions): the diff is complete, so the unchanged
   * lines between hunks expand without a request
   */
  withContents?: boolean
}

/**
 * One patch of a file from the hunks of a `FileDiff`: a whole `diff --git` patch (a commit file, an uncommitted file,
 * a missing commit), or hunks that each repeat the `---`/`+++` header (the change of a commit in a modify/delete conflict). The header is
 * kept once; a hunk list without one gets `--- a/<old>` / `+++ b/<new>`.
 */
export function joinFileDiffHunks(diff: FileDiff): string {
  const lines: string[] = []
  let hasHeader = false
  for (const hunk of diff.hunks) {
    const hunkLines = hunk.split("\n")
    const firstHunkLine = hunkLines.findIndex(line => line.startsWith("@@"))
    const header = firstHunkLine === -1 ? hunkLines : hunkLines.slice(0, firstHunkLine)
    if (!hasHeader) {
      if (!header.some(line => line.startsWith("--- ") || line.startsWith("diff --git "))) {
        lines.push(`--- a/${diff.oldFile.fileName}`, `+++ b/${diff.newFile.fileName}`)
      }
      lines.push(...header)
      hasHeader = true
    }
    if (firstHunkLine !== -1) {
      const body = hunkLines.slice(firstHunkLine)
      // a patch ends with a newline: its last split part is empty, not a line
      if (body.at(-1) === "") {
        body.pop()
      }
      lines.push(...body)
    }
  }
  return `${lines.join("\n")}\n`
}

/** The parsed diff of a file for `FileDiffView`; `undefined` when the patch has no file in it */
export function toFileDiffMetadata(diff: FileDiff, options: ToFileDiffMetadataOptions = {}): FileDiffMetadata | undefined {
  const patch = joinFileDiffHunks(diff)
  return processFile(patch, {
    cacheKey: options.cacheKey,
    isGitDiff: patch.startsWith("diff --git "),
    oldFile: options.withContents ? { name: diff.oldFile.fileName, contents: diff.oldFile.content } : undefined,
    newFile: options.withContents ? { name: diff.newFile.fileName, contents: diff.newFile.content } : undefined,
    throwOnError: false,
  })
}
