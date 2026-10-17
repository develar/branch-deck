import { Spinner } from "@/components/ui/spinner"
import { Callout } from "@/components/shared/Callout"
import type { ConflictContentsEntry } from "@/stores/conflict-contents"

export interface DiffLoadStateProps {
  /** What is loading: "Loading file content…" */
  loading: string
  /** Why the load failed, the title of a destructive callout; null while it loads */
  error: string | null
}

/** What an open file item shows until its diff is loaded: a spinner with what loads, or why it did not load */
export function DiffLoadState({ loading, error }: DiffLoadStateProps) {
  if (error !== null) {
    return (
      <div className="p-4">
        <Callout tone="destructive" title={error} />
      </div>
    )
  }
  return (
    <p className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
      <Spinner className="size-4" />
      {loading}
    </p>
  )
}

/** `DiffLoadState` of an open item of a conflict view until the contents of the conflict are loaded */
export function ConflictContentsLoadState({ entry }: { entry: ConflictContentsEntry | undefined }) {
  return <DiffLoadState loading="Loading the conflict…" error={entry?.kind === "error" ? `Cannot load the conflict: ${entry.message}` : null} />
}
