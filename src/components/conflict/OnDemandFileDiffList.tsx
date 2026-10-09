// TODO(Phase 7): placeholder; replaced by the full port of OnDemandFileDiffList.vue (accordion of files, diff loaded on expand).
import { Badge } from "@/components/ui/badge"
import type { UncommittedFileChange } from "@/lib/bindings"
import type { ConflictDiffViewMode } from "@/stores/settings/conflictViewer"

export interface OnDemandFileDiffListProps {
  files: UncommittedFileChange[]
  keyPrefix: string
  diffViewMode: ConflictDiffViewMode
}

export function OnDemandFileDiffList({ files, keyPrefix }: OnDemandFileDiffListProps) {
  return (
    <div className="flex flex-col gap-2 bd-padding-card">
      {files.map((file, index) => (
        <div key={`${keyPrefix}-${index}`} className="flex items-center justify-between gap-2 rounded-lg border border-border px-3 py-2">
          <span className="truncate font-mono text-sm">{file.filePath}</span>
          <Badge variant="secondary">{file.status}</Badge>
        </div>
      ))}
    </div>
  )
}
