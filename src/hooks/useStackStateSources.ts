import { useMemo } from "react"
import { useIsFullyLanded } from "@/hooks/useLandedIn"
import type { StackStateSources } from "@/lib/stack-state"
import { useFailingRobots, usePushHolds, useRepositoryRobots, useRobotsNeedingReview } from "@/stores/patronus"

/**
 * What the state counts of a stack read besides its branches (`summarizeStackState`), for the stack chip and the island
 * header; the same object while the robots, the landing and the push holds stay
 */
export function useStackStateSources(): Required<StackStateSources> {
  const robots = useRepositoryRobots()
  const needsReview = useRobotsNeedingReview()
  const failing = useFailingRobots()
  const isLanded = useIsFullyLanded()
  const holds = usePushHolds()
  return useMemo(() => ({ robots, needsReview, failing, isLanded, isHeld: (name: string) => holds.has(name) }), [robots, needsReview, failing, isLanded, holds])
}
