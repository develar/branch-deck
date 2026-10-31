import { createHash } from "node:crypto"
import type { BlobText, CommitFile } from "../../src/lib/bindings"
import type { DiffBlock, FileDiff } from "../../src/lib/diff/file-diff-model"

// Synthetic commits for the diff benchmark: Kotlin-like files whose lines each hold several tokens (a keyword, names,
// a number, a string, a comment), so a highlighted line differs from a plain one, with the diffs of the engine

/** A file of a synthetic commit: its row in the files, its diff as the backend sends it, the blobs of both sides */
export interface SyntheticFile {
  file: CommitFile
  diff: FileDiff
  blobs: BlobText[]
}

export interface FileSpec {
  path: string
  lines: number
  /** Every `changeEvery`-th line changes (1:1); 0 for a new file */
  changeEvery: number
}

function codeLine(fileIndex: number, index: number, version: number): string {
  return `  val value${index} = compute(${index + version}, "file${fileIndex}") // line ${index}`
}

/** The offsets of the number argument of `codeLine`: the range the engine marks in a changed line, a modified one */
function numberRange(line: string): [number, number, number] {
  const start = line.indexOf("(") + 1
  return [start, line.indexOf(",", start), 2]
}

function blobId(path: string, side: string): string {
  return createHash("sha1").update(`${side}:${path}`).digest("hex")
}

/** A modified file with 1:1 changes, or a new file, as the engine sends its diff */
export function syntheticFile(fileIndex: number, spec: FileSpec): SyntheticFile {
  const oldLines = spec.changeEvery === 0 ? [] : Array.from({ length: spec.lines }, (_, index) => codeLine(fileIndex, index, 0))
  const newLines = Array.from({ length: spec.lines }, (_, index) => codeLine(fileIndex, index, spec.changeEvery !== 0 && index % spec.changeEvery === spec.changeEvery - 1 ? 1 : 0))
  const oldId = spec.changeEvery === 0 ? null : blobId(spec.path, "old")
  const newId = blobId(spec.path, "new")
  const blocks: DiffBlock[] =
    spec.changeEvery === 0
      ? [{ oldStart: 0, oldEnd: 0, newStart: 0, newEnd: spec.lines, kind: "changed", inlinePairs: false, oldRanges: [], newRanges: [] }]
      : newLines.flatMap((line, index) =>
          index % spec.changeEvery === spec.changeEvery - 1
            ? [
                {
                  oldStart: index,
                  oldEnd: index + 1,
                  newStart: index,
                  newEnd: index + 1,
                  kind: "changed",
                  inlinePairs: true,
                  oldRanges: [numberRange(oldLines[index]!)],
                  newRanges: [numberRange(line)],
                },
              ]
            : [],
        )
  const changed = spec.changeEvery === 0 ? 0 : blocks.length
  const additions = spec.changeEvery === 0 ? spec.lines : changed
  const text = (lines: string[]) => `${lines.join("\n")}\n`
  const blobs: BlobText[] = [{ kind: "text", id: newId, text: text(newLines) }]
  if (oldId !== null) {
    blobs.push({ kind: "text", id: oldId, text: text(oldLines) })
  }
  return {
    file: { path: spec.path, status: spec.changeEvery === 0 ? "added" : "modified", oldPath: null, additions, deletions: changed, binary: false, oldId, newId, submodule: false },
    diff: {
      path: spec.path,
      oldPath: null,
      status: spec.changeEvery === 0 ? "added" : "modified",
      oldId,
      newId,
      // as the backend's `collapse_of` says it: over 1,000 changed lines a file starts collapsed
      collapse: additions + changed > 1000 ? { kind: "large", changed: additions + changed } : null,
      content: {
        kind: "text",
        // the engine counts the empty line after the final line end, and the one line of an empty text
        oldLineCount: oldLines.length + 1,
        newLineCount: newLines.length + 1,
        additions,
        deletions: changed,
        lineEnds: null,
        finalNewline: null,
        emptyLastLine: { old: true, new: true },
        lineLevelOnly: false,
        blocks,
        oldScopes: [],
        newScopes: [{ startLine: 0, endLine: spec.lines, label: `class File${fileIndex}` }],
        importBlocks: [],
      },
    },
    blobs,
  }
}

/**
 * A large commit: 200 files in 20 directories, most of them small with a few changes, 4 of 2,000-5,000 lines with a
 * change every 12 lines, and 10 new files
 */
export function largeCommit(): SyntheticFile[] {
  const specs: FileSpec[] = []
  for (let index = 0; index < 200; index++) {
    const directory = `module${index % 20}/src/main/kotlin`
    if (index % 50 === 7) {
      specs.push({ path: `${directory}/Large${index}.kt`, lines: 2000 + (index % 4) * 1000, changeEvery: 12 })
    } else if (index % 20 === 3) {
      specs.push({ path: `${directory}/New${index}.kt`, lines: 40, changeEvery: 0 })
    } else {
      specs.push({ path: `${directory}/File${index}.kt`, lines: 80 + (index % 7) * 20, changeEvery: 25 })
    }
  }
  return sortByPath(specs).map((spec, index) => syntheticFile(index, spec))
}

/** A commit of one file of 10,000 lines with a change every 20 lines */
export function hugeFileCommit(): SyntheticFile[] {
  return [syntheticFile(0, { path: "core/src/main/kotlin/Huge.kt", lines: 10_000, changeEvery: 20 })]
}

function sortByPath(specs: FileSpec[]): FileSpec[] {
  return specs.toSorted((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0))
}
