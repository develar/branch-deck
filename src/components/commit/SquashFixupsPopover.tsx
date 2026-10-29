import { useEffect, useState } from "react"
import { CombineIcon, CornerDownRightIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { PopoverDescription, PopoverTitle } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { LinkedText } from "@/components/commit/LinkedText"
import { LockWaitNotice } from "@/components/shared/LockWaitNotice"
import { squashFixups, squashFixupsSurface } from "@/hooks/useSquashFixups"
import { describeSquashFixups, formatFixupCount, getAutosquashScope, getFixupMarker, getListFixups, getOrphanSubject } from "@/lib/autosquash"
import type { ListFixups } from "@/lib/autosquash"
import type { AutosquashPlan, PlannedGroup } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { UNASSIGNED_SOURCE } from "@/stores/commit-move"
import { useIndicatorClaim, useSurfaceBusy } from "@/stores/indicators"
import { withRowProgress } from "@/stores/inline-row-action"
import { getSelectedProject } from "@/stores/repository"

export interface SquashFixupsPopoverProps {
  /** The branch whose fixups are squashed; null for the unassigned commits */
  branch: string | null
  onClose: () => void
}

/**
 * Confirmation of squashing the fixups of a list: each target with the fixups folded into it, then the fixups without a
 * target, which stay. The plan is read again when it opens, and the squash is refused if the branch moved since.
 * The squash runs as an operation of the list started here (the popover stays open with its spinner meanwhile), then
 * closes it.
 */
export function SquashFixupsPopover({ branch, onClose }: SquashFixupsPopoverProps) {
  const [plan, setPlan] = useState<AutosquashPlan | null>(null)
  const key = branch ?? UNASSIGNED_SOURCE
  const surface = squashFixupsSurface(key)
  useIndicatorClaim(surface)
  const isSquashing = useSurfaceBusy(surface) !== null
  useEffect(() => {
    let cancelled = false
    void invokeCommand(() => commands.getAutosquashPlan({ repositoryPath: getSelectedProject()?.path ?? "" })).then(result => {
      if (cancelled) {
        return
      }
      if (result.status === "ok") {
        setPlan(result.data)
      } else {
        notifyError("Cannot list the fixups", result.error)
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
        <Spinner className="size-3" />
        Reading the fixups…
      </div>
    )
  }

  const fixups: ListFixups = getListFixups(plan, branch)
  // closes the popover after a failure too: the plan it shows is stale
  const squash = () => void withRowProgress(key, () => squashFixups(getAutosquashScope(branch), { surface }))
  const description = describeSquashFixups(fixups.count)
  return (
    <>
      <div className="flex flex-col gap-1">
        <PopoverTitle className="text-sm">{fixups.count === 0 ? "No fixups to squash" : `Squash ${formatFixupCount(fixups.count)}`}</PopoverTitle>
        {description !== null && <PopoverDescription className="text-xs">{description}</PopoverDescription>}
      </div>

      {fixups.groups.length > 0 && (
        <ul className="flex max-h-72 flex-col gap-2 overflow-y-auto" data-testid="squash-fixups-groups">
          {fixups.groups.map(group => (
            <FixupGroup key={group.target} group={group} />
          ))}
        </ul>
      )}

      {fixups.orphans.length > 0 && (
        <div className="flex flex-col gap-1" data-testid="squash-fixups-orphans">
          <span className="text-xs text-warning">Won't be squashed: no target in this range</span>
          <ul className="flex flex-col gap-0.5">
            {fixups.orphans.map(orphan => {
              const { marker, text } = getOrphanSubject(orphan)
              return (
                <li key={orphan.commitId} className="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                  <span className="shrink-0 font-mono">{marker}</span>
                  <LinkedText text={text} className="truncate" />
                </li>
              )
            })}
          </ul>
        </div>
      )}

      <LockWaitNotice surface={surface} />
      <div className="flex justify-end gap-2">
        <Button size="xs" variant="ghost" disabled={isSquashing} onClick={onClose}>
          Cancel
        </Button>
        <Button size="xs" disabled={fixups.count === 0 || isSquashing} onClick={squash} data-testid="squash-fixups-confirm">
          {isSquashing ? <Spinner data-icon="inline-start" /> : <CombineIcon data-icon="inline-start" />}
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
      <LinkedText text={group.targetParts.rest} className="truncate text-sm font-medium" />
      {group.folded.map(fixup => (
        <span key={fixup.commitId} className="flex min-w-0 items-center gap-1.5 pl-1 text-xs text-muted-foreground">
          <CornerDownRightIcon className="size-3 shrink-0" />
          <span className="shrink-0 font-mono">{getFixupMarker(fixup.kind)}</span>
          <LinkedText text={fixup.subjectParts.rest} className="truncate" />
        </span>
      ))}
    </li>
  )
}
