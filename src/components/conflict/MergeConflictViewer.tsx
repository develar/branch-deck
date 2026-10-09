// TODO(Phase 7): placeholder; replaced by the full port of MergeConflictViewer.vue (conflict overview, missing commits, file diffs, 3-way merge view).
import { GitMergeIcon } from "lucide-react"
import type { MergeConflictInfo } from "@/lib/bindings"

export interface MergeConflictViewerProps {
  conflict: MergeConflictInfo
  branchName?: string
}

export function MergeConflictViewer({ conflict }: MergeConflictViewerProps) {
  return (
    <div className="flex flex-col gap-4">
      <div className="overflow-hidden rounded-xl bg-card p-4 ring-1 ring-foreground/10">
        <div className="flex items-start gap-3">
          <GitMergeIcon className="mt-0.5 size-5 shrink-0 text-destructive" />
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <p className="text-sm font-medium text-foreground">Cherry-pick conflict detected</p>
            <p className="text-sm text-muted-foreground">
              Cannot apply:
              {" "}
              {conflict.commitMessage}
            </p>
            <ul className="list-inside list-disc text-xs text-muted-foreground">
              {conflict.conflictingFiles.map(file => <li key={file.file}>{file.file}</li>)}
            </ul>
          </div>
        </div>
      </div>
    </div>
  )
}
