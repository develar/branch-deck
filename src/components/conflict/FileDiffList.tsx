import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { LazyFileDiffView } from "@/components/diff/LazyDiffViews"
import type { ConflictCommit, FileDiff } from "@/lib/bindings"
import type { ConflictDiffViewMode } from "@/stores/settings/conflictViewer"

export interface FileDiffListProps {
  fileDiffs: readonly FileDiff[]
  keyPrefix: string
  conflictMarkerCommits?: Record<string, ConflictCommit>
  diffViewMode?: ConflictDiffViewMode
  /** A muted line above the diff of the file at the same index ("No conflict markers in this file: …") */
  fileNotes?: readonly (string | null)[]
  /** The file contents of each diff agree with its hunks (a missing commit), so the unchanged lines expand */
  withContents?: boolean
}

/** Accordion of file diffs (collapsed by default); conflict markers in added lines get an annotation. */
export function FileDiffList({ fileDiffs, keyPrefix, conflictMarkerCommits, diffViewMode = "unified", fileNotes, withContents = false }: FileDiffListProps) {
  return (
    <AccordionPrimitive.Root multiple className="flex flex-col gap-2">
      {fileDiffs.map((diff, index) => (
        <AccordionPrimitive.Item key={`${keyPrefix}-${index}`} value={`item-${index}`} className="overflow-hidden rounded-lg border border-border" data-testid="file-diff">
          <CollapsibleFileHeader fileName={diff.newFile.fileName} />
          <AccordionPrimitive.Panel className="border-t border-border">
            {fileNotes?.[index] && <p className="border-b border-border px-3 py-2 text-xs text-muted-foreground">{fileNotes[index]}</p>}
            {/* mounted only while the item is open */}
            <LazyFileDiffView diff={diff} diffViewMode={diffViewMode} withContents={withContents} conflictMarkers conflictMarkerCommits={conflictMarkerCommits} />
          </AccordionPrimitive.Panel>
        </AccordionPrimitive.Item>
      ))}
    </AccordionPrimitive.Root>
  )
}
