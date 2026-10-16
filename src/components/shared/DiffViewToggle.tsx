import { cn } from "cn"
import { AlignLeftIcon, Columns2Icon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { setConflictDiffViewMode, useConflictViewerSettings } from "@/stores/settings/conflictViewer"
import type { ConflictDiffViewMode } from "@/stores/settings/conflictViewer"

const modes: Array<{ mode: ConflictDiffViewMode; label: string; icon: typeof AlignLeftIcon }> = [
  { mode: "unified", label: "Unified", icon: AlignLeftIcon },
  { mode: "split", label: "Split", icon: Columns2Icon },
]

/** Unified/split switch for diff views, persisted in the conflict viewer settings. */
export function DiffViewToggle() {
  const current = useConflictViewerSettings(state => state.conflictDiffViewMode)
  return (
    <FieldGroup>
      {modes.map(({ mode, label, icon: Icon }) => (
        <Button
          key={mode}
          size="xs"
          variant="outline"
          aria-pressed={current === mode}
          className={cn(current === mode && "border-primary/50 text-primary hover:text-primary")}
          onClick={() => setConflictDiffViewMode(mode)}
        >
          <Icon data-icon="inline-start" />
          {label}
        </Button>
      ))}
    </FieldGroup>
  )
}
