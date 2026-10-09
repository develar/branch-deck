import { useMemo } from "react"
import { AlignLeftIcon, Columns3Icon } from "lucide-react"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { FileDiffList } from "@/components/conflict/FileDiffList"
import { GitDiffMergeView } from "@/components/conflict/GitDiffMergeView"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import type { ConflictDetail, ConflictMarkerCommitInfo, FileDiff, MergeConflictInfo } from "@/lib/bindings"
import { setConflictViewMode, setShowConflictsOnly, useConflictViewerSettings } from "@/stores/settings/conflictViewer"

export interface ConflictingFilesSectionProps {
  conflicts: readonly ConflictDetail[]
  conflictInfo?: MergeConflictInfo
  conflictMarkerCommits?: Record<string, ConflictMarkerCommitInfo>
}

/** Conflicting files as diffs with conflict markers (optionally only the conflicting hunks) or as a 3-way merge view. */
export function ConflictingFilesSection({ conflicts, conflictInfo, conflictMarkerCommits }: ConflictingFilesSectionProps) {
  const viewMode = useConflictViewerSettings(state => state.viewMode)
  const showConflictsOnly = useConflictViewerSettings(state => state.showConflictsOnly)
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)

  const fileDiffs = useMemo(() => conflicts.map(file => file.fileDiff), [conflicts])
  const conflictOnlyFileDiffs = useMemo(() => filterConflictHunks(fileDiffs), [fileDiffs])

  // Base UI marks the panels with `data-hidden` only; `data-state` is what the E2E tests read
  const panelState = (value: string) => (viewMode === value ? "active" : "inactive")

  return (
    <div className="flex flex-col gap-3">
      <Tabs value={viewMode} onValueChange={value => setConflictViewMode(String(value))} className="gap-4">
        <TabsList variant="line">
          <TabsTrigger value="diff">
            <AlignLeftIcon />
            Diff View
          </TabsTrigger>
          <TabsTrigger value="3way">
            <Columns3Icon />
            3-way Merge
          </TabsTrigger>
        </TabsList>

        <TabsContent value="diff" data-state={panelState("diff")}>
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between px-1">
              <label className="flex cursor-pointer items-center gap-2 text-sm font-medium text-foreground">
                <Switch size="sm" checked={showConflictsOnly} onCheckedChange={checked => setShowConflictsOnly(checked)} />
                Conflicts only
              </label>
              <DiffViewToggle />
            </div>

            <FileDiffList
              fileDiffs={showConflictsOnly ? conflictOnlyFileDiffs : fileDiffs}
              keyPrefix="conflict"
              conflictMarkerCommits={conflictMarkerCommits}
              diffViewMode={diffViewMode}
            />
          </div>
        </TabsContent>

        <TabsContent value="3way" data-state={panelState("3way")}>
          <GitDiffMergeView conflicts={conflicts} conflictInfo={conflictInfo} />
        </TabsContent>
      </Tabs>
    </div>
  )
}

function hasConflictMarkers(hunk: string): boolean {
  return hunk.includes("<<<<<<<") || hunk.includes("|||||||") || hunk.includes("=======") || hunk.includes(">>>>>>>")
}

/** Keeps only the hunks with conflict markers, and only the files that have such hunks */
function filterConflictHunks(fileDiffs: readonly FileDiff[]): FileDiff[] {
  return fileDiffs
    .map(fileDiff => ({ ...fileDiff, hunks: fileDiff.hunks.filter(hasConflictMarkers) }))
    .filter(fileDiff => fileDiff.hunks.length > 0)
}
