import { useEffect } from "react"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import type { StatsPeriod } from "@/lib/bindings"
import { useBranchSync } from "@/stores/branch-sync"
import { useEffectiveBranchPrefix, useSelectedProjectPath } from "@/stores/repository"
import type { RobotStatsEntry, RobotStatsSubject } from "@/stores/robot-stats"
import { readRobotStats, useRobotStatsEntry } from "@/stores/robot-stats"
import { setStatsPeriod, useAppSettings } from "@/stores/settings/appSettings"

export interface RobotStatsView extends RobotStatsEntry {
  /** The period the user chose; the entry may still hold the numbers of another one while its read runs */
  readonly period: StatsPeriod
  readonly setPeriod: (period: StatsPeriod) => void
  readonly refresh: () => void
}

/**
 * The Safe Push statistics of the selected repository in the chosen period, read while `visible`: when the view shows,
 * when the period changes and when the window gets the focus (`readRobotStats`). A hidden view reads nothing.
 */
export function useRobotStats(visible: boolean): RobotStatsView {
  const repositoryPath = useSelectedProjectPath() ?? null
  const branchPrefix = useEffectiveBranchPrefix()
  const baselineBranch = useBranchSync(state => state.baselineBranch)
  const period = useAppSettings(state => state.statsPeriod)
  const subject: RobotStatsSubject | null = repositoryPath === null || baselineBranch === null || branchPrefix === "" ? null : { repositoryPath, baselineBranch, period }
  const entry = useRobotStatsEntry(repositoryPath)
  useEffect(() => {
    if (visible && repositoryPath !== null && baselineBranch !== null && branchPrefix !== "") {
      // Rust names the user's robots by the prefix it resolves; a new prefix reads them again
      readRobotStats({ repositoryPath, baselineBranch, period }, false)
    }
  }, [visible, repositoryPath, baselineBranch, branchPrefix, period])
  useWindowFocus(() => {
    if (visible && subject !== null) {
      readRobotStats(subject, false)
    }
  })
  return {
    ...entry,
    period,
    setPeriod: setStatsPeriod,
    refresh: () => {
      if (subject !== null) {
        readRobotStats(subject, true)
      }
    },
  }
}
