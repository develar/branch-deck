import { useEffect, useMemo, useState } from "react"
import type { ReactNode } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import type { DeclarationPlace, DiffViewElsewhere } from "@/components/diff/DiffView"
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
 * the two texts of its file by blob id. A note of a diff that names a place of another file of the list (the other
 * end of a move between two files, the file that declares a renamed name) opens the item of that file and shows the
 * place in its diff.
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
  // the items that are open, each by its value: the user's, and the one a note of another diff of the list asked for
  const [open, setOpen] = useState<readonly string[]>(() => defaultOpen.map(index => `item-${index}`))
  // the place such a note named, for the diff of its file
  const [shownPlace, setShownPlace] = useState<DeclarationPlace | null>(null)
  const elsewhere: DiffViewElsewhere = {
    shows: path => files.some(file => file.path === path),
    show: place => {
      const value = `item-${files.findIndex(file => file.path === place.path)}`
      setOpen(previous => (previous.includes(value) ? previous : [...previous, value]))
      // a new object each time: the same place asked again is shown again
      setShownPlace({ ...place })
    },
  }

  return (
    <AccordionPrimitive.Root
      multiple
      value={[...open]}
      onValueChange={(value: string[]) => {
        setOpen(value)
        // an item the user closes and opens again shows the top of its diff: the place a note asked of it is asked no more
        setShownPlace(previous => (previous !== null && !value.includes(`item-${files.findIndex(file => file.path === previous.path)}`) ? null : previous))
      }}
      className="flex flex-col gap-2"
    >
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
                <SubjectFileDiff
                  file={file}
                  diff={diff}
                  diffViewMode={diffViewMode}
                  worktree={subject.kind === "uncommitted"}
                  elsewhere={elsewhere}
                  shownPlace={shownPlace?.path === file.path ? shownPlace : undefined}
                />
              )}
            </AccordionPrimitive.Panel>
          </AccordionPrimitive.Item>
        )
      })}
    </AccordionPrimitive.Root>
  )
}

export interface SubjectFileDiffProps {
  file: SubjectFile
  diff: FileDiff | undefined
  diffViewMode: DiffViewMode
  worktree?: boolean
  /** The diffs of the other files of the subject beside this one: what shows a place of another file that a note names */
  elsewhere?: DiffViewElsewhere
  /** A place of this file that a note of another diff named: shown once the diff is there, and again when it is another object */
  shownPlace?: DeclarationPlace
}

/**
 * The diff of an open item in `DiffView` without a header, in the scroller of what holds it: its rows virtualized, its
 * texts read by blob id as they come into view, its notice in a row
 */
export function SubjectFileDiff({ file, diff, diffViewMode, worktree = false, elsewhere, shownPlace }: SubjectFileDiffProps) {
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
  return (
    <LazyDiffView
      files={files}
      mode={diffViewMode}
      softWraps={softWraps}
      initialPath={null}
      label={`Changes of ${file.path}`}
      scroll="parent"
      worktree={worktree}
      elsewhere={elsewhere}
      shownPlace={shownPlace}
    />
  )
}
