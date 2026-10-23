import { Spinner } from "@/components/ui/spinner"
import { SubjectFileDiffList } from "@/components/conflict/SubjectFileDiffList"
import type { SubjectFile } from "@/components/conflict/SubjectFileDiffList"
import { Callout } from "@/components/shared/Callout"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { NestedPanel } from "@/components/shared/NestedPanel"
import type { UncommittedChangesResult, UncommittedFileChange } from "@/lib/bindings"
import { parseFileStatus } from "@/lib/commit-files"
import { useSelectedProjectPath } from "@/stores/repository"
import { useDiffViewMode } from "@/stores/settings/diff-view-settings"

export interface UncommittedChangesCardProps {
  diffData: UncommittedChangesResult | null
  loading: boolean
  error: string | null
}

/** Preview of the uncommitted changes in the amend form, a panel nested in the expanded row. */
export function UncommittedChangesCard({ diffData, loading, error }: UncommittedChangesCardProps) {
  const diffViewMode = useDiffViewMode()
  // the working tree of the repository the form shows: another one's uncommitted changes are another entry
  const repositoryPath = useSelectedProjectPath() ?? ""
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
            {/* a new answer of the uncommitted changes reads their diffs again */}
            <SubjectFileDiffList
              subject={{ kind: "uncommitted", repositoryPath }}
              files={toDiffListFiles(diffData.files)}
              keyPrefix="uncommitted"
              diffViewMode={diffViewMode}
              refreshKey={diffData}
            />
          </div>
        )
      )}
    </NestedPanel>
  )
}

function toDiffListFiles(files: readonly UncommittedFileChange[]): SubjectFile[] {
  return files.map(file => ({ path: file.filePath, status: parseFileStatus(file.status) }))
}
