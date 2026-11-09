import { useRef, useState } from "react"
import type { RefObject } from "react"
import { RefreshCwIcon, ScissorsIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { useDiskUsage } from "@/hooks/useDiskUsage"
import { trimPools } from "@/hooks/useTrimPools"
import type { MachineDisk, PoolDisk } from "@/lib/bindings"
import {
  diskProgress,
  formatSize,
  freeSpaceLine,
  isBelowMinimum,
  machineTitle,
  measuredAge,
  poolLine,
  slotSizeDetail,
  slotsBySize,
  slotStateLabel,
  trimDisabledReason,
  trimWhat,
} from "@/presenters/disk"
import { closeDiskUsageDialog, useDiskUsageDialog } from "@/stores/disk-usage-dialog"

/**
 * Disk Usage (openspec/specs/repository/disk-usage/spec.md): what the lease pools of this Mac and of the hosts of `bd-host` hold,
 * each slot with the bytes its builds wrote, the slots a trim deletes next marked, and Trim to Limit for this Mac.
 * Opened by "Show Disk Usage…" of the command palette; kept mounted while closed, as the frame needs.
 */
export function DiskUsageDialog() {
  const { repositoryPath, generation } = useDiskUsageDialog()
  // the last repository stays while the closing animation runs
  const [shown, setShown] = useState<{ repositoryPath: string; generation: number } | null>(null)
  if (repositoryPath !== null && shown?.generation !== generation) {
    setShown({ repositoryPath, generation })
  }
  const bodyRef = useRef<HTMLDivElement>(null)
  return (
    <PreviewDialogFrame
      data-testid="disk-usage-dialog"
      width="lg"
      open={repositoryPath !== null}
      onOpenChange={nextOpen => {
        if (!nextOpen) {
          closeDiskUsageDialog()
        }
      }}
      initialFocus={bodyRef}
    >
      {shown !== null && <DiskUsage key={shown.generation} repositoryPath={shown.repositoryPath} generation={shown.generation} bodyRef={bodyRef} />}
    </PreviewDialogFrame>
  )
}

function DiskUsage({ repositoryPath, generation, bodyRef }: { repositoryPath: string; generation: number; bodyRef: RefObject<HTMLDivElement | null> }) {
  const { state, refreshing, reload } = useDiskUsage(repositoryPath, generation)
  const [trimming, setTrimming] = useState(false)
  const thisMac = state.status === "loaded" ? state.overview.machines.find(machine => machine.alias === null) : undefined
  const disabledReason = thisMac === undefined ? "Wait for the disk usage" : trimDisabledReason(thisMac)
  return (
    <>
      <PreviewDialogHeader
        end={
          <Button variant="ghost" size="sm" disabled={state.status === "loading" || refreshing} onClick={() => reload(true)}>
            {refreshing ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
            Measure Again
          </Button>
        }
      >
        <h2 className="text-base font-semibold">Disk Usage</h2>
        <p className="text-sm text-muted-foreground">
          What a delete of each slot of the lease pools frees: its own worktree blocks, its build directories and its Bazel output bases.
        </p>
      </PreviewDialogHeader>
      <PreviewDialogBody ref={bodyRef} data-testid="disk-usage-body">
        {state.status === "loading" && (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Spinner />
            Measuring the lease pools…
          </div>
        )}
        {state.status === "error" && (
          <div className="flex flex-col items-start gap-3 text-sm">
            <p role="alert">Cannot measure the lease pools: {state.message}</p>
            <Button variant="outline" size="sm" onClick={() => reload(false)}>
              Retry
            </Button>
          </div>
        )}
        {state.status === "loaded" && (
          <div className="flex flex-col gap-6">
            {refreshing && (
              <p role="status" className="text-sm text-muted-foreground">
                {diskProgress(state.measured, state.measuring)}
              </p>
            )}
            {state.overview.machines.map(machine => (
              <Machine key={machine.alias ?? ""} machine={machine} refreshing={refreshing} />
            ))}
          </div>
        )}
      </PreviewDialogBody>
      <PreviewDialogFooter start={thisMac !== undefined && disabledReason === null ? `Trim deletes ${trimWhat(thisMac)} on this Mac` : disabledReason}>
        <Button
          size="sm"
          disabled={disabledReason !== null || trimming}
          onClick={() => {
            setTrimming(true)
            void trimPools(repositoryPath).then(summary => {
              setTrimming(false)
              if (summary !== null) {
                reload(false)
              }
            })
          }}
        >
          {trimming ? <Spinner data-icon="inline-start" /> : <ScissorsIcon data-icon="inline-start" />}
          Trim to Limit
        </Button>
      </PreviewDialogFooter>
    </>
  )
}

function Machine({ machine, refreshing }: { machine: MachineDisk; refreshing: boolean }) {
  const title = machineTitle(machine)
  const measuring = refreshing && machine.alias === null
  return (
    <section aria-label={title} className="flex flex-col gap-3">
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
        <h3 className="text-sm font-semibold">{title}</h3>
        {machine.error === null && <span className={isBelowMinimum(machine) ? "text-sm text-warning" : "text-sm text-muted-foreground"}>{freeSpaceLine(machine)}</span>}
      </div>
      {machine.error !== null && <p className="text-sm text-muted-foreground">{machine.error}</p>}
      {machine.error === null && machine.pools.length === 0 && <p className="text-sm text-muted-foreground">No lease pools</p>}
      {machine.pools.map(pool => (
        <Pool key={pool.path} pool={pool} refreshing={measuring} />
      ))}
      {machine.orphans > 0 && (
        <p className="text-sm text-muted-foreground">
          {machine.orphans} Bazel output {machine.orphans === 1 ? "base" : "bases"} of gone slots: {formatSize(machine.orphanMib)}
        </p>
      )}
    </section>
  )
}

function Pool({ pool, refreshing }: { pool: PoolDisk; refreshing: boolean }) {
  return (
    <div role="group" aria-label={pool.name} className="flex flex-col gap-1 rounded-lg border border-border p-3">
      <div className="flex flex-wrap items-baseline justify-between gap-x-4">
        <span className="font-medium">{pool.name}</span>
        <span className="text-sm text-muted-foreground">
          {formatSize(pool.mib)} · {poolLine(pool)}
        </span>
      </div>
      {pool.checkoutBasesMib > 0 && <span className="text-xs text-muted-foreground">Bazel output bases of the main checkout: {formatSize(pool.checkoutBasesMib)}</span>}
      <ul className="flex flex-col">
        {slotsBySize(pool).map(slot => (
          <li
            key={slot.path}
            aria-busy={refreshing && slot.cached}
            className={`flex items-center gap-3 py-1 text-sm ${refreshing && slot.cached ? "bg-muted" : ""}`}
            title={slot.path}
          >
            <span className="w-8 shrink-0 text-muted-foreground tabular-nums">{slot.name}</span>
            <span className="min-w-0 flex-1 truncate">
              {slotStateLabel(slot)} · {measuredAge(slot.measuredAt)}
            </span>
            {slot.trimsNext && <Badge variant="outline">Goes with a trim</Badge>}
            <span className="hidden text-xs text-muted-foreground sm:inline">{slotSizeDetail(slot)}</span>
            <span className="w-20 shrink-0 text-right tabular-nums">{formatSize(slot.mib)}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}
