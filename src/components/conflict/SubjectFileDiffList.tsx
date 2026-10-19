import { useEffect, useMemo } from "react"
import type { ReactNode } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { LazyDiffView } from "@/components/diff/LazyDiffViews"
import { Callout } from "@/components/shared/Callout"
import { LoadingLine } from "@/components/shared/LoadingLine"
import type { CommitFile, FileStatus } from "@/lib/bindings"
import type { FileDiff, FileDiffSubject } from "@/lib/diff/file-diff-model"
import { withEngineCounts } from "@/lib/diff/view-files"
import { fileDiffOf, loadFileDiffs, useFileDiffs } from "@/stores/file-diffs"
import { useDiffSoftWraps, useDiffWhitespacePolicy } from "@/stores/settings/diff-view-settings"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/** A file of the list: its path, and its status when the list knows it (a conflict view lists paths only) */
export interface SubjectFile {
  path: string
  status?: FileStatus
}

export interface SubjectFileDiffListProps {
  /** Whose diffs: a missing commit, the commit of a modify/delete conflict, the uncommitted changes */
  subject: FileDiffSubject
  files: readonly SubjectFile[]
  keyPrefix: string
  diffViewMode: DiffViewMode
  /** A new value reads the diffs again: the uncommitted changes of the working tree changed */
  refreshKey?: unknown
  /** The items open at first; all closed when absent */
  defaultOpen?: readonly number[]
  /** Above the diff of an open item, e.g. why a modify/delete conflict shows the change of the commit */
  renderBeforeDiff?: (file: SubjectFile) => ReactNode
  /** At the end of a header, before the counts, e.g. the conflict count */
  renderActions?: (file: SubjectFile) => ReactNode
}

/**
 * An accordion of the changed files of a subject, collapsed by default, from the diffs of the engine (one
 * `get_file_diffs` for the subject under the whitespace policy): a header with the engine's counts, an open item reads
 * the two texts of its file by blob id.
 */
export function SubjectFileDiffList({ subject, files, keyPrefix, diffViewMode, refreshKey, defaultOpen = [], renderBeforeDiff, renderActions }: SubjectFileDiffListProps) {
  const policy = useDiffWhitespacePolicy()
  const entry = useFileDiffs(subject, policy)
  const subjectKey = JSON.stringify(subject)
  // the uncommitted changes are read again with every refresh key; a commit never changes
  useEffect(() => {
    const current = JSON.parse(subjectKey) as FileDiffSubject
    loadFileDiffs(current, policy, { refresh: current.kind === "uncommitted" })
  }, [subjectKey, policy, refreshKey])

  return (
    <AccordionPrimitive.Root multiple defaultValue={defaultOpen.map(index => `item-${index}`)} className="flex flex-col gap-2">
      {files.map((file, index) => {
        const found = fileDiffOf(entry, file.path)
        const diff = found === "pending" ? undefined : found
        const counts = diff?.content.kind === "text" ? diff.content : null
        return (
          <AccordionPrimitive.Item key={`${keyPrefix}-${index}`} value={`item-${index}`} className="overflow-hidden rounded-lg border border-border" data-testid="file-diff">
            <CollapsibleFileHeader
              path={file.path}
              oldPath={diff?.oldPath ?? null}
              status={file.status ?? diff?.status}
              actions={
                <>
                  {renderActions?.(file)}
                  {counts !== null && (counts.additions > 0 || counts.deletions > 0) && <LineCounts additions={counts.additions} deletions={counts.deletions} colored />}
                </>
              }
            />
            <AccordionPrimitive.Panel className="border-t border-border">
              {/* mounted only while the item is open */}
              {renderBeforeDiff?.(file)}
              {entry?.kind === "error" ? (
                <div className="p-4">
                  <Callout tone="destructive" title="Cannot read the diff">
                    {entry.message}
                  </Callout>
                </div>
              ) : found === "pending" ? (
                <LoadingLine className="p-4">Loading diff…</LoadingLine>
              ) : (
                <SubjectFileDiff file={file} diff={diff} diffViewMode={diffViewMode} worktree={subject.kind === "uncommitted"} />
              )}
            </AccordionPrimitive.Panel>
          </AccordionPrimitive.Item>
        )
      })}
    </AccordionPrimitive.Root>
  )
}

/**
 * The diff of an open item in `DiffView` without a header, in the scroller of what holds it: its rows virtualized, its
 * texts read by blob id as they come into view, its notice in a row
 */
export function SubjectFileDiff({ file, diff, diffViewMode, worktree = false }: { file: SubjectFile; diff: FileDiff | undefined; diffViewMode: DiffViewMode; worktree?: boolean }) {
  const softWraps = useDiffSoftWraps()
  const files = useMemo(() => {
    const commitFile: CommitFile = {
      path: file.path,
      status: file.status ?? diff?.status ?? "modified",
      oldPath: diff?.oldPath ?? null,
      additions: 0,
      deletions: 0,
      binary: diff?.content.kind === "binary",
      oldId: diff?.oldId ?? null,
      newId: diff?.newId ?? null,
      submodule: diff?.content.kind === "submodule",
    }
    return withEngineCounts([commitFile], diff === undefined ? undefined : [diff]).map(counted => ({ file: counted, diff }))
  }, [file, diff])
  return <LazyDiffView files={files} mode={diffViewMode} softWraps={softWraps} initialPath={null} label={`Changes of ${file.path}`} scroll="parent" worktree={worktree} />
}

/** The diff of one file of a subject on its own, e.g. the change of the commit of a modify/delete conflict */
export function SubjectFileDiffPanel({ subject, path, diffViewMode }: { subject: FileDiffSubject; path: string; diffViewMode: DiffViewMode }) {
  const policy = useDiffWhitespacePolicy()
  const entry = useFileDiffs(subject, policy)
  const subjectKey = JSON.stringify(subject)
  useEffect(() => {
    loadFileDiffs(JSON.parse(subjectKey) as FileDiffSubject, policy)
  }, [subjectKey, policy])
  if (entry?.kind === "error") {
    return (
      <div className="p-4">
        <Callout tone="destructive" title="Cannot read the diff">
          {entry.message}
        </Callout>
      </div>
    )
  }
  const diff = fileDiffOf(entry, path)
  if (diff === "pending") {
    return <LoadingLine className="p-4">Loading diff…</LoadingLine>
  }
  return <SubjectFileDiff file={{ path }} diff={diff} diffViewMode={diffViewMode} worktree={subject.kind === "uncommitted"} />
}
