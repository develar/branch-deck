import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { Badge } from "@/components/ui/badge"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { LazyConflictFileView, LazyFileDiffView } from "@/components/diff/LazyDiffViews"
import type { ConflictDetail } from "@/lib/bindings"
import { countConflicts, labelConflictMarkers } from "@/lib/merge-conflict"
import type { ConflictMarkerLabels } from "@/lib/merge-conflict"
import { useConflictViewerSettings } from "@/stores/settings/diff-view-settings"

export interface ConflictingFilesSectionProps {
  conflicts: readonly ConflictDetail[]
  /** What the marker lines name: the branch the commit is copied onto, the version before it, the commit */
  labels: ConflictMarkerLabels
}

/**
 * The conflicting files, one item each, the first open: the conflicts of a file as an unresolved file of the diff
 * engine (the change of the branch the commit is copied onto against the change of the commit, a few lines of context);
 * a file without conflict markers (a modify/delete conflict) shows the change of the commit, with a line that says so.
 */
export function ConflictingFilesSection({ conflicts, labels }: ConflictingFilesSectionProps) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  return (
    <AccordionPrimitive.Root multiple defaultValue={conflicts.length === 0 ? [] : ["file-0"]} className="flex flex-col gap-2">
      {conflicts.map((conflict, index) => {
        const contents = conflict.merged.content
        const count = countConflicts(contents)
        return (
          <AccordionPrimitive.Item key={`${index}-${conflict.file}`} value={`file-${index}`} className="overflow-hidden rounded-lg border border-border" data-testid="file-diff">
            <CollapsibleFileHeader
              fileName={conflict.file}
              actions={
                <Badge variant="outline" className="tabular-nums" data-testid="file-conflict-count">
                  {count === 0 ? "modify/delete" : count === 1 ? "1 conflict" : `${count} conflicts`}
                </Badge>
              }
            />
            <AccordionPrimitive.Panel className="border-t border-border">
              {/* mounted only while the item is open */}
              {count > 0 ? (
                <LazyConflictFileView name={conflict.file} contents={labelConflictMarkers(contents, labels)} />
              ) : (
                <>
                  <p className="border-b border-border px-3 py-2 text-xs text-muted-foreground">One side deleted the file, the other changed it: the change of the commit</p>
                  <LazyFileDiffView diff={conflict.commitDiff} diffViewMode={diffViewMode} withContents />
                </>
              )}
            </AccordionPrimitive.Panel>
          </AccordionPrimitive.Item>
        )
      })}
    </AccordionPrimitive.Root>
  )
}
