import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { OnDemandFileDiffList } from "@/components/conflict/OnDemandFileDiffList"
import type { OnDemandFile } from "@/components/conflict/OnDemandFileDiffList"
import { Callout } from "@/components/shared/Callout"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { NestedPanel } from "@/components/shared/NestedPanel"
import { commands } from "@/lib/bindings"
import type { UncommittedChangesResult, UncommittedFileChange } from "@/lib/bindings"
import { getFileStatusColor } from "@/lib/commit-files"
import { subtleBadgeVariant } from "@/lib/ui-types"
import { vcsRequestFactory } from "@/stores/repository"
import { useConflictViewerSettings } from "@/stores/settings/diff-view-settings"

export interface UncommittedChangesCardProps {
  diffData: UncommittedChangesResult | null
  loading: boolean
  error: string | null
}

/** Preview of the uncommitted changes in the amend form, a panel nested in the expanded row. */
export function UncommittedChangesCard({ diffData, loading, error }: UncommittedChangesCardProps) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  const hasChanges = !loading && !error && diffData?.hasChanges === true

  return (
    <NestedPanel
      data-testid="uncommitted-changes-card"
      header={{
        title: "Uncommitted Changes",
        count: diffData?.files.length ?? 0,
        itemSingular: "file",
        itemPlural: "files",
        actions: hasChanges && <DiffViewToggle />,
      }}
    >
      {loading ? (
        <p className="flex items-center gap-2 bd-padding-content text-sm text-muted-foreground">
          <Spinner className="size-4" />
          Loading uncommitted changes…
        </p>
      ) : error ? (
        <div className="bd-padding-content">
          <Callout tone="destructive" title={error} />
        </div>
      ) : !diffData?.hasChanges ? (
        <p className="bd-padding-content text-sm text-muted-foreground">No uncommitted changes to amend</p>
      ) : (
        diffData.files.length > 0 && (
          <div className="bd-padding-card">
            <OnDemandFileDiffList files={toDiffListFiles(diffData.files)} keyPrefix="uncommitted" diffViewMode={diffViewMode} loadDiff={loadWorkingTreeDiff} />
          </div>
        )
      )}
    </NestedPanel>
  )
}

function toDiffListFiles(files: readonly UncommittedFileChange[]): OnDemandFile[] {
  return files.map(file => ({
    path: file.filePath,
    actions: <Badge variant={subtleBadgeVariant(getFileStatusColor(file.status))}>{file.status}</Badge>,
  }))
}

/** The diff of a file against HEAD, from the working tree */
function loadWorkingTreeDiff(file: OnDemandFile) {
  const { repositoryPath } = vcsRequestFactory.createRequest()
  return commands.getFileContentForDiff({ repositoryPath, filePath: file.path })
}
