import { CircleAlertIcon, InfoIcon } from "lucide-react"
import { Card } from "@/components/ui/card"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { OnDemandFileDiffList } from "@/components/conflict/OnDemandFileDiffList"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import type { UncommittedChangesResult } from "@/lib/bindings"
import { useConflictViewerSettings } from "@/stores/settings/conflictViewer"

export interface UncommittedChangesCardProps {
  diffData: UncommittedChangesResult | null
  loading: boolean
  error: string | null
}

/** Preview of the uncommitted changes in the amend form. */
export function UncommittedChangesCard({ diffData, loading, error }: UncommittedChangesCardProps) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  const hasChanges = !loading && !error && diffData?.hasChanges === true

  return (
    <Card className="gap-0 overflow-hidden py-0" data-testid="uncommitted-changes-card">
      <div className="border-b border-border px-4 py-3 sm:px-6">
        <SectionCardHeader
          title="Uncommitted Changes"
          count={diffData?.files.length ?? 0}
          itemSingular="file"
          itemPlural="files"
          actions={(
            <div className="flex items-center gap-3">
              <Popover>
                <PopoverTrigger openOnHover nativeButton={false} render={<span className="inline-flex" />}>
                  <InfoIcon className="size-3.5 cursor-pointer text-muted-foreground transition-colors hover:text-foreground" />
                </PopoverTrigger>
                <PopoverContent className="w-xs gap-2 p-3 text-xs">
                  <p>This will amend your uncommitted changes to the tip commit of this virtual branch.</p>
                  <p>The original commit message and author information will be preserved.</p>
                </PopoverContent>
              </Popover>
              {hasChanges && <DiffViewToggle />}
            </div>
          )}
        />
      </div>

      {loading
        ? (
            <div className="flex items-center justify-center py-8">
              <div className="flex items-center gap-3">
                <Spinner className="size-4 text-muted-foreground" />
                <span className="text-sm text-muted-foreground">Loading uncommitted changes…</span>
              </div>
            </div>
          )
        : error
          ? (
              <div className="bd-padding-card">
                <div className="flex items-center gap-2 rounded-md border border-destructive/20 bg-destructive/10 p-4">
                  <CircleAlertIcon className="size-4 text-destructive" />
                  <span className="text-sm text-destructive">{error}</span>
                </div>
              </div>
            )
          : !diffData?.hasChanges
              ? (
                  <div className="bd-padding-card">
                    <div className="flex items-center gap-2 rounded-md border border-border bg-muted/50 p-4">
                      <InfoIcon className="size-4 text-muted-foreground" />
                      <span className="text-sm text-muted-foreground">No uncommitted changes to amend</span>
                    </div>
                  </div>
                )
              : diffData.files.length > 0 && <OnDemandFileDiffList files={diffData.files} keyPrefix="uncommitted" diffViewMode={diffViewMode} />}
    </Card>
  )
}
