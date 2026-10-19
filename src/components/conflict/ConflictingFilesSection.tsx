import { useEffect, useState } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { DiffLoadState } from "@/components/conflict/DiffLoadState"
import { LazyDiffView } from "@/components/diff/LazyDiffViews"
import { FieldGroup } from "@/components/layout/FieldGroup"
import type { CommitFile, ConflictFile } from "@/lib/bindings"
import type { FileDiff } from "@/lib/diff/file-diff-model"
import type { FileMerge, MergeMiddle, MergeSource } from "@/lib/diff/file-merge-model"
import { mergeContentNotice } from "@/lib/diff/view-files"
import { fileMergeOf, loadFileMerges, useFileMerges } from "@/stores/file-merges"
import { useDiffSoftWraps, useDiffViewMode, useDiffWhitespacePolicy } from "@/stores/settings/diff-view-settings"

/** What the columns of a merge name */
export interface MergeColumnLabels {
  /** Left, ours: the branch the commit is copied onto */
  left: string
  /** Right, theirs: the commit */
  right: string
}

export interface ConflictingFilesSectionProps {
  files: readonly ConflictFile[]
  /** The cherry-pick that conflicts: its merges come from `get_file_merges` */
  source: MergeSource
  labels: MergeColumnLabels
}

const MIDDLES: Array<{ middle: MergeMiddle; label: string }> = [
  { middle: "result", label: "Result (proposed)" },
  { middle: "base", label: "Base" },
]

/**
 * The conflicting files, one item each, the first open: the merge of each file in three columns, as IntelliJ's merge
 * viewer shows it, read-only (docs/design/diff-view.md, "The 3-way view"). The middle column shows the result that the
 * engine proposes, or the base. A file that one side deleted shows the change of the other side. The headers come
 * with the conflict; the merges stream in, the open file first.
 */
export function ConflictingFilesSection({ files, source, labels }: ConflictingFilesSectionProps) {
  const policy = useDiffWhitespacePolicy()
  const entry = useFileMerges(source, policy)
  const [middle, setMiddle] = useState<MergeMiddle>("result")
  const { parent, target, commit } = source
  const first = files[0]?.file
  useEffect(() => loadFileMerges({ kind: "cherryPick", parent, target, commit }, policy, first === undefined ? [] : [first]), [parent, target, commit, policy, first])

  return (
    <AccordionPrimitive.Root multiple defaultValue={files.length === 0 ? [] : ["file-0"]} className="flex flex-col gap-2">
      {files.map((file, index) => {
        const count = file.conflictCount
        const found = fileMergeOf(entry, file.file)
        return (
          <AccordionPrimitive.Item key={`${index}-${file.file}`} value={`file-${index}`} className="overflow-hidden rounded-lg border border-border" data-testid="file-diff">
            <CollapsibleFileHeader
              path={file.file}
              actions={
                <Badge variant="outline" className="tabular-nums" data-testid="file-conflict-count">
                  {count === 0 ? "modify/delete" : count === 1 ? "1 conflict" : `${count} conflicts`}
                </Badge>
              }
            />
            <AccordionPrimitive.Panel className="border-t border-border">
              {/* mounted only while the item is open */}
              {found === "pending" ? (
                <DiffLoadState loading="Loading the conflict…" error={null} />
              ) : found === undefined ? (
                <DiffLoadState loading="" error={entry?.kind === "error" ? `Cannot load the conflict: ${entry.message}` : "Git reports no conflict in this file"} />
              ) : (
                <FileMergeView merge={found} labels={labels} middle={middle} onMiddleChange={setMiddle} />
              )}
            </AccordionPrimitive.Panel>
          </AccordionPrimitive.Item>
        )
      })}
    </AccordionPrimitive.Root>
  )
}

interface FileMergeViewProps {
  merge: FileMerge
  labels: MergeColumnLabels
  middle: MergeMiddle
  onMiddleChange: (middle: MergeMiddle) => void
}

/** The merge of one file under the names of its three columns; a one-sided merge as the diff of the change it keeps */
function FileMergeView({ merge, labels, middle, onMiddleChange }: FileMergeViewProps) {
  const mode = useDiffViewMode()
  const softWraps = useDiffSoftWraps()
  const { content } = merge
  if (content.kind === "oneSided") {
    // the diff of the base to the side that kept the file
    const newId = content.deleted === "left" ? merge.rightId : merge.leftId
    const diff: FileDiff = { path: merge.path, oldPath: null, status: "modified", oldId: merge.baseId, newId, content: { kind: "text", ...content.diff } }
    const file: CommitFile = { path: merge.path, status: "modified", oldPath: null, additions: 0, deletions: 0, binary: false, oldId: merge.baseId, newId, submodule: false }
    return (
      <>
        <p className="border-b border-border px-3 py-2 text-xs text-muted-foreground" data-testid="merge-one-sided">
          {content.deleted === "left"
            ? `${labels.left} does not have the file, the commit changes it: the change of the commit`
            : `The commit deletes the file, ${labels.left} changed it: that change`}
        </p>
        <LazyDiffView files={[{ file, diff }]} mode={mode} softWraps={softWraps} initialPath={null} label={`Change of ${merge.path}`} scroll="parent" />
      </>
    )
  }
  if (content.kind !== "text") {
    return <p className="px-3 py-2 text-xs text-muted-foreground">{mergeContentNotice(content)}</p>
  }
  return (
    <>
      <div className="grid grid-cols-3 items-center gap-2 border-b border-border px-3 py-1 text-xs text-muted-foreground" data-testid="merge-columns">
        <span className="truncate" title={labels.left}>
          {labels.left}
        </span>
        <FieldGroup className="justify-self-center">
          {MIDDLES.map(choice => (
            <Button
              key={choice.middle}
              size="xs"
              variant="outline"
              aria-pressed={middle === choice.middle}
              className={cn(middle === choice.middle && "border-primary/50 text-primary hover:text-primary")}
              onClick={() => onMiddleChange(choice.middle)}
            >
              {choice.label}
            </Button>
          ))}
        </FieldGroup>
        <span className="justify-self-end truncate" title={labels.right}>
          {labels.right}
        </span>
      </div>
      <LazyDiffView files={[{ path: merge.path, merge }]} mode={mode} middle={middle} softWraps={softWraps} initialPath={null} label={`Merge of ${merge.path}`} scroll="parent" />
    </>
  )
}
