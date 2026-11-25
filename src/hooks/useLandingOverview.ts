import { useMemo } from "react"
import { getLandingNodeNow, getNodeKindNow, nodeSources, useNodeStores } from "@/hooks/useLandingNode"
import { useRobotOverview } from "@/hooks/useRobotOverview"
import type { LandingOverview, LandingUnit } from "@/lib/landing-overview"
import { collectLandingOverview } from "@/lib/landing-overview"
import { branchSyncStore } from "@/stores/branch-sync"

/**
 * The units of the landing plan now, as Rust decides them (`RepositoryView.landingUnits`, `domain_policy::landing_plan`),
 * in the order of the rows: a lone branch, or the root of a stack with its branches, each with the node Rust names in
 * the page's words; a branch of a stack that needs a fix of its own comes with it while the move that runs on it
 * leaves it one
 */
function computeLandingUnits(): LandingUnit[] {
  const { landingUnits, branchOrder } = branchSyncStore.getState()
  const units: LandingUnit[] = []
  for (const name of branchOrder) {
    const unit = landingUnits[name]
    const node = unit === undefined ? null : getLandingNodeNow(unit.node)
    if (unit === undefined || node === null) {
      continue
    }
    // what runs on a row is laid over its node: a fix whose move runs is none
    const fixes = unit.fixes.flatMap(fix => {
      const fixNode = getNodeKindNow(fix) === "needsFix" ? getLandingNodeNow(fix) : null
      return fixNode === null ? [] : [{ name: fix, node: fixNode }]
    })
    units.push({ name: unit.name, branchCount: unit.branches.length, commitCount: unit.commitCount, node, fixes })
  }
  return units
}

let shown: { sources: readonly unknown[]; key: string; units: LandingUnit[] } | null = null

/** The units now, computed once per change of the stores the nodes read, the same array while they stay */
export function getLandingUnitsNow(): LandingUnit[] {
  const sources = nodeSources()
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
  const units = useNodeStores(getLandingUnitsNow)
  const robots = useRobotOverview()
  return useMemo(() => collectLandingOverview(units, robots), [units, robots])
}
