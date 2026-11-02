import { lazy, Suspense } from "react"
import type { ReactElement } from "react"
import { CombineIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { squashFixupsSurface, useListFixups } from "@/hooks/useSquashFixups"
import { formatFixupCount, formatSquashFixupsLabel } from "@/lib/autosquash"
import { UNASSIGNED_SOURCE } from "@/stores/commit-move"
import { useSurfaceBusy } from "@/stores/indicators"
import { closeInline, openInline, useActiveInlineForRow } from "@/stores/inline-row-action"

const loadSquashFixupsPopover = () => import("@/components/commit/SquashFixupsPopover")
const SquashFixupsPopover = lazy(() => loadSquashFixupsPopover().then(it => ({ default: it.SquashFixupsPopover })))

/**
 * "Squash N Fixups…" for the fixups of the unassigned commits whose target is among them, in the header of their card;
 * opens the confirmation listing what goes where. Nothing without such fixups. Its xs button overhangs the 20px line of
 * the header into the padding, so the header stays 40px with it.
 */
export function SquashFixupsButton() {
  return (
    <SquashFixups
      branch={null}
      trigger={count => (
        <PopoverTrigger render={<Button size="xs" variant="ghost" className="-my-0.5" data-testid="squash-fixups-button" onPointerEnter={() => void loadSquashFixupsPopover()} />}>
          <CombineIcon data-icon="inline-start" />
          {formatSquashFixupsLabel(count)}
        </PopoverTrigger>
      )}
    />
  )
}

/**
 * "N fixups" after the name of a branch whose fixups have their target in it: a press opens the confirmation of
 * "Squash N Fixups…", which its tooltip names, also with the commits collapsed; the branch menu opens it too
 * (`openInline`). Nothing without such fixups.
 */
export function BranchFixupBadge({ branch }: { branch: string }) {
  return (
    <SquashFixups
      branch={branch}
      trigger={(count, open) => (
        <Tooltip lazy>
          <PopoverTrigger
            render={
              <TooltipTrigger
                // the open confirmation says it already
                disabled={open}
                render={
                  <Badge
                    variant="outline"
                    render={<button type="button" />}
                    className="shrink-0 cursor-pointer text-muted-foreground outline-none hover:bg-muted hover:text-foreground"
                    data-testid="branch-fixup-count"
                    onPointerEnter={() => void loadSquashFixupsPopover()}
                    // the row expands on a click
                    onClick={event => event.stopPropagation()}
                  />
                }
              />
            }
          >
            {formatFixupCount(count)}
          </PopoverTrigger>
          <TooltipContent>{formatSquashFixupsLabel(count)}</TooltipContent>
        </Tooltip>
      )}
    />
  )
}

/** The confirmation of squashing the fixups of a list (a branch, or the unassigned commits: null), opened by `trigger` */
function SquashFixups({ branch, trigger }: { branch: string | null; trigger: (count: number, open: boolean) => ReactElement }) {
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
      {trigger(count, open)}
      <PopoverContent align={branch === null ? "end" : "start"} className="w-96 gap-3 p-3" data-testid="squash-fixups-popover">
        <Suspense fallback={null}>
          <SquashFixupsPopover branch={branch} onClose={closeInline} />
        </Suspense>
      </PopoverContent>
    </Popover>
  )
}
