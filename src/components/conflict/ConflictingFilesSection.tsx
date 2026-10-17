import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { Badge } from "@/components/ui/badge"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { ConflictContentsLoadState } from "@/components/conflict/DiffLoadState"
import { LazyConflictFileView, LazyFileDiffView } from "@/components/diff/LazyDiffViews"
import type { ConflictDetail, ConflictFile } from "@/lib/bindings"
import { labelConflictMarkers } from "@/lib/merge-conflict"
import type { ConflictMarkerLabels } from "@/lib/merge-conflict"
import type { ConflictContentsEntry } from "@/stores/conflict-contents"
import type { ConflictDiffViewMode } from "@/stores/settings/diff-view-settings"
import { useConflictViewerSettings } from "@/stores/settings/diff-view-settings"

export interface ConflictingFilesSectionProps {
  files: readonly ConflictFile[]
  /** The contents of the conflict (`useConflictContents`): the merged file and the change of the commit of each file */
  contents: ConflictContentsEntry | undefined
  /** What the marker lines name: the branch the commit is copied onto, the version before it, the commit */
  labels: ConflictMarkerLabels
}

/**
 * The conflicting files, one item each, the first open: the conflicts of a file as an unresolved file of the diff
 * engine (the change of the branch the commit is copied onto against the change of the commit, a few lines of context);
 * a file without conflict markers (a modify/delete conflict) shows the change of the commit, with a line that says so.
 * The headers come with the conflict, the files with its contents.
 */
export function ConflictingFilesSection({ files, contents, labels }: ConflictingFilesSectionProps) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  return (
    <AccordionPrimitive.Root multiple defaultValue={files.length === 0 ? [] : ["file-0"]} className="flex flex-col gap-2">
      {files.map((file, index) => {
        const count = file.conflictCount
        const detail = contents?.kind === "loaded" ? contents.contents.conflictingFiles[index] : undefined
        return (
          <AccordionPrimitive.Item key={`${index}-${file.file}`} value={`file-${index}`} className="overflow-hidden rounded-lg border border-border" data-testid="file-diff">
            <CollapsibleFileHeader
              fileName={file.file}
              actions={
                <Badge variant="outline" className="tabular-nums" data-testid="file-conflict-count">
                  {count === 0 ? "modify/delete" : count === 1 ? "1 conflict" : `${count} conflicts`}
                </Badge>
              }
            />
            <AccordionPrimitive.Panel className="border-t border-border">
              {/* mounted only while the item is open */}
              {count === 0 && (
                <p className="border-b border-border px-3 py-2 text-xs text-muted-foreground">One side deleted the file, the other changed it: the change of the commit</p>
              )}
              {detail?.file === file.file ? (
                <ConflictingFileView detail={detail} count={count} labels={labels} diffViewMode={diffViewMode} />
              ) : (
                <ConflictContentsLoadState entry={contents} />
              )}
            </AccordionPrimitive.Panel>
          </AccordionPrimitive.Item>
        )
      })}
    </AccordionPrimitive.Root>
  )
}

function ConflictingFileView({ detail, count, labels, diffViewMode }: { detail: ConflictDetail; count: number; labels: ConflictMarkerLabels; diffViewMode: ConflictDiffViewMode }) {
  return count > 0 ? (
    <LazyConflictFileView name={detail.file} contents={labelConflictMarkers(detail.merged.content, labels)} />
  ) : (
    <LazyFileDiffView diff={detail.commitDiff} diffViewMode={diffViewMode} withContents />
  )
}
