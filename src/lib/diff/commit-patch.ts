import { processFile } from "@pierre/diffs"
import type { FileContents, FileDiffContentsLoader, FileDiffMetadata } from "@pierre/diffs"
import type { BlobContents, CommitFile, CommitPatch, FilePatch } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { getDiffRepositoryPath } from "@/stores/repository"

/** A file of a commit with its parsed diff, or why it shows none */
export interface CommitFileDiff {
  file: CommitFile
  /** Without hunks for a binary file, a mode change or a pure rename */
  fileDiff: FileDiffMetadata | undefined
  /** The hunk lines the patch left out: the file passed the size limits (`PatchLimits` in `commit_files.rs`) */
  omittedLines: number | null
}

// parsed once per patch: Pierre fills a parsed diff in place when it expands the unchanged lines
const parsedPatches = new WeakMap<CommitPatch, ReadonlyMap<string, ParsedSection>>()

interface ParsedSection {
  fileDiff: FileDiffMetadata | undefined
  omittedLines: number | null
}

/** The sections of a patch by the path after the commit, the first one for a path (a type change prints two) */
function parseSections(patch: CommitPatch): ReadonlyMap<string, ParsedSection> {
  let sections = parsedPatches.get(patch)
  if (sections === undefined) {
    const byPath = new Map<string, ParsedSection>()
    patch.files.forEach((section, index) => {
      const fileDiff = parseSection(section, `${patch.commitId}:${index}`)
      if (fileDiff !== undefined && !byPath.has(fileDiff.name)) {
        byPath.set(fileDiff.name, { fileDiff, omittedLines: section.omittedLines })
      }
    })
    sections = byPath
    parsedPatches.set(patch, sections)
  }
  return sections
}

function parseSection(section: FilePatch, cacheKey: string): FileDiffMetadata | undefined {
  return processFile(section.patch, { cacheKey, isGitDiff: true, throwOnError: false })
}

/** The diff of a file of the commit; a file the patch has no section for has no diff */
export function getCommitFileDiff(patch: CommitPatch, file: CommitFile): CommitFileDiff {
  const section = parseSections(patch).get(file.path)
  return { file, fileDiff: section?.fileDiff, omittedLines: section?.omittedLines ?? null }
}

/** The diffs of the files, in their order */
export function getCommitFileDiffs(patch: CommitPatch, files: readonly CommitFile[]): CommitFileDiff[] {
  return files.map(file => getCommitFileDiff(patch, file))
}

/** An all-zero id: the side a file does not exist on */
function isNullObjectId(id: string | undefined): boolean {
  return id === undefined || /^0+$/.test(id)
}

/**
 * Reads both sides of a file by the blob ids of its `index` line (`--full-index`), with one `get_blob_contents`, when
 * the diff expands the unchanged lines. Pierre only logs a failed load, so it is shown here.
 */
export const loadCommitFileContents: FileDiffContentsLoader = async fileDiff => {
  const repositoryPath = getDiffRepositoryPath() ?? ""
  const ids = [fileDiff.prevObjectId, fileDiff.newObjectId].filter((id): id is string => !isNullObjectId(id))
  const result = await invokeCommand(() => commands.getBlobContents({ repositoryPath, objectIds: ids }))
  const title = `Cannot show the unchanged lines of ${fileDiff.name}`
  if (result.status === "error") {
    notifyError(title, result.error)
    throw new Error(result.error.message)
  }
  const byId = new Map(result.data.map(blob => [blob.objectId, blob]))
  const contents = (name: string, id: string | undefined): FileContents | null => {
    if (isNullObjectId(id)) {
      return null
    }
    const blob = byId.get(id!)
    if (blob?.kind !== "text") {
      const reason = describeUnreadableBlob(blob)
      notifyError(title, reason)
      throw new Error(reason)
    }
    return { name, contents: blob.content, cacheKey: blob.objectId }
  }
  const oldFile = contents(fileDiff.prevName ?? fileDiff.name, fileDiff.prevObjectId)
  const newFile = contents(fileDiff.name, fileDiff.newObjectId) ?? { name: fileDiff.name, contents: "" }
  return { oldFile, newFile }
}

function describeUnreadableBlob(blob: BlobContents | undefined): string {
  switch (blob?.kind) {
    case "binary":
      return "the file is binary"
    case "tooLarge":
      return `the file is too large (${Math.round(blob.size / 1024 / 1024)} MB)`
    default:
      return "the file is not in the repository"
  }
}
