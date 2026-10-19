import { lazy, Suspense } from "react"
import { CombineIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { squashFixupsSurface, useListFixups } from "@/hooks/useSquashFixups"
import { formatSquashFixupsLabel } from "@/lib/autosquash"
import { UNASSIGNED_SOURCE } from "@/stores/commit-move"
import { closeInline, openInline, useActiveInlineForRow } from "@/stores/inline-row-action"
import { useSurfaceBusy } from "@/stores/operations"

const loadSquashFixupsPopover = () => import("@/components/commit/SquashFixupsPopover")
const SquashFixupsPopover = lazy(() => loadSquashFixupsPopover().then(it => ({ default: it.SquashFixupsPopover })))

/**
 * "Squash N Fixups…" for the fixups whose target is in the list (a branch, or the unassigned commits: null); opens the
 * confirmation listing what goes where. Nothing without such fixups. The branch menu opens it too (`openInline`).
 */
export function SquashFixupsButton({ branch }: { branch: string | null }) {
  const { count } = useListFixups(branch)
  const key = branch ?? UNASSIGNED_SOURCE
  const activeInline = useActiveInlineForRow(key)
  const open = activeInline?.type === "squash-fixups"
  const squashing = useSurfaceBusy(squashFixupsSurface(key)) !== null
  if (count === 0) {
    return null
  }

  return (
    <Popover
      open={open}
      onOpenChange={nextOpen => {
        if (nextOpen) {
          openInline("squash-fixups", key)
        }
        // nothing closes it while the squash runs (as the squash dialog): the squash closes it
        else if (open && !squashing) {
          closeInline()
        }
      }}
    >
      <PopoverTrigger render={<Button size="xs" variant="ghost" data-testid="squash-fixups-button" onPointerEnter={() => void loadSquashFixupsPopover()} />}>
        <CombineIcon data-icon="inline-start" />
        {formatSquashFixupsLabel(count)}
      </PopoverTrigger>
      <PopoverContent align="end" className="w-96 gap-3 p-3" data-testid="squash-fixups-popover">
        <Suspense fallback={null}>
          <SquashFixupsPopover branch={branch} onClose={closeInline} />
        </Suspense>
      </PopoverContent>
    </Popover>
  )
}
