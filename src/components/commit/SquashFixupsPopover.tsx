import { useEffect, useState } from "react"
import { CombineIcon, CornerDownRightIcon, LoaderCircleIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { PopoverDescription, PopoverTitle } from "@/components/ui/popover"
import { LinkedText } from "@/components/commit/LinkedText"
import { squashFixups } from "@/hooks/useSquashFixups"
import { formatFixupCount, getAutosquashScope, getDisplaySubject, getFixupMarker, getListFixups } from "@/lib/autosquash"
import type { ListFixups } from "@/lib/autosquash"
import type { AutosquashPlan, PlannedGroup } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { parseSubject } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { getSelectedProject } from "@/stores/repository"

export interface SquashFixupsPopoverProps {
  /** The branch whose fixups are squashed; null for the unassigned commits */
  branch: string | null
  onClose: () => void
}

/**
 * Confirmation of squashing the fixups of a list: each target with the fixups folded into it, then the fixups without a
 * target, which stay. The plan is read again when it opens, and the squash is refused if the branch moved since.
 */
export function SquashFixupsPopover({ branch, onClose }: SquashFixupsPopoverProps) {
  const [plan, setPlan] = useState<AutosquashPlan | null>(null)
  const [isSquashing, setIsSquashing] = useState(false)
  useEffect(() => {
    let cancelled = false
    void invokeCommand(() => commands.getAutosquashPlan({ repositoryPath: getSelectedProject()?.path ?? "" })).then((result) => {
      if (cancelled) {
        return
      }
      if (result.status === "ok") {
        setPlan(result.data)
      }
      else {
        notifyError("Failed to List Fixups", result.error)
        onClose()
      }
    })
    return () => {
      cancelled = true
    }
  }, [onClose])

  if (plan === null) {
    return (
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <LoaderCircleIcon className="size-3 animate-spin" />
        Reading the fixups…
      </div>
    )
  }

  const fixups: ListFixups = getListFixups(plan, branch)
  const squash = () => {
    setIsSquashing(true)
    void squashFixups(getAutosquashScope(branch), plan.head).then(() => onClose())
  }
  return (
    <>
      <div className="flex flex-col gap-1">
        <PopoverTitle className="text-sm">{fixups.count === 0 ? "No fixups to squash" : `Squash ${formatFixupCount(fixups.count)}`}</PopoverTitle>
        <PopoverDescription className="text-xs">Each one is folded into its target, as with rebase --autosquash. Your files are not touched.</PopoverDescription>
      </div>

      {fixups.groups.length > 0 && (
        <ul className="flex max-h-72 flex-col gap-2 overflow-y-auto" data-testid="squash-fixups-groups">
          {fixups.groups.map(group => <FixupGroup key={group.target} group={group} />)}
        </ul>
      )}

      {fixups.orphans.length > 0 && (
        <div className="flex flex-col gap-1" data-testid="squash-fixups-orphans">
          <span className="text-xs text-warning">Won't be squashed: no target in this range</span>
          <ul className="flex flex-col gap-0.5">
            {fixups.orphans.map(orphan => (
              <li key={orphan.commitId} className="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                <span className="shrink-0 font-mono">{parseSubject(orphan.subject).autosquash.trim()}</span>
                <LinkedText text={getDisplaySubject(orphan.subject)} className="truncate" />
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="flex justify-end gap-2">
        <Button size="sm" variant="ghost" onClick={onClose}>Cancel</Button>
        <Button size="sm" disabled={fixups.count === 0 || isSquashing} onClick={squash} data-testid="squash-fixups-confirm">
          {isSquashing ? <LoaderCircleIcon data-icon="inline-start" className="animate-spin" /> : <CombineIcon data-icon="inline-start" />}
          Squash
        </Button>
      </div>
    </>
  )
}

/** The target, then each fixup with its marker */
function FixupGroup({ group }: { group: PlannedGroup }) {
  return (
    <li className="flex min-w-0 flex-col gap-0.5">
      <LinkedText text={getDisplaySubject(group.targetSubject)} className="truncate text-sm font-medium" />
      {group.folded.map(fixup => (
        <span key={fixup.commitId} className="flex min-w-0 items-center gap-1.5 pl-1 text-xs text-muted-foreground">
          <CornerDownRightIcon className="size-3 shrink-0" />
          <span className="shrink-0 font-mono">{getFixupMarker(fixup.kind)}</span>
          <LinkedText text={getDisplaySubject(fixup.subject)} className="truncate" />
        </span>
      ))}
    </li>
  )
}
