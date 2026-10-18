import { useMemo } from "react"
import { nextStepSources, useNextStepStores } from "@/hooks/useBranchNextStep"
import { getLandingNodeNow, getStackLandingNodeNow } from "@/hooks/useLandingNode"
import { useRobotOverview } from "@/hooks/useRobotOverview"
import type { LandingOverview, LandingUnit } from "@/lib/landing-overview"
import { collectLandingOverview } from "@/lib/landing-overview"
import { getBranch, getBranchTableRows, getStackDescendants } from "@/stores/branch-sync"

/** The units of the table now, in its order: a lone branch, or the root of a stack with its branches */
function computeLandingUnits(): LandingUnit[] {
  const units: LandingUnit[] = []
  for (const row of getBranchTableRows()) {
    // a root: no line up into its node, no fork into it
    if (row.kind !== "branch" || row.lanes.up || row.lanes.forkFrom !== null) {
      continue
    }
    const members = [row.name, ...getStackDescendants(row.name)]
    const node = members.length > 1 ? getStackLandingNodeNow(row.name) : getLandingNodeNow(row.name)
    if (node === null) {
      continue
    }
    const fixes =
      members.length > 1
        ? members.flatMap(name => {
            const own = getLandingNodeNow(name)
            return own?.kind === "needsFix" ? [{ name, node: own }] : []
          })
        : []
    const commitCount = members.reduce((sum, name) => sum + (getBranch(name)?.commitCount ?? 0), 0)
    units.push({ name: row.name, branchCount: members.length, commitCount, node, fixes })
  }
  return units
}

let shown: { sources: readonly unknown[]; key: string; units: LandingUnit[] } | null = null

/** The units now, computed once per change of the stores the next step reads, the same array while they stay */
export function getLandingUnitsNow(): LandingUnit[] {
  const sources = nextStepSources()
  if (shown !== null && shown.sources.every((source, index) => source === sources[index])) {
    return shown.units
  }
  const units = computeLandingUnits()
  const key = JSON.stringify(units)
  shown = { sources, key, units: shown?.key === key ? shown.units : units }
  return shown.units
}

/**
 * Where the work is on its way into master, for the header (`LandingIndicator`): the units of the table by their
 * landing nodes, beside the robots of the branches, the whole stacks and the watched ones (`useRobotOverview`)
 */
export function useLandingOverview(): LandingOverview {
  const units = useNextStepStores(getLandingUnitsNow)
  const robots = useRobotOverview()
  return useMemo(() => collectLandingOverview(units, robots), [units, robots])
}
