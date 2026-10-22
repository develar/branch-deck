import { useEffect } from "react"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { useBranchSync } from "@/stores/branch-sync"
import { useEffectiveBranchPrefix, useSelectedProjectPath } from "@/stores/repository"
import type { RobotStatsEntry, RobotStatsSubject } from "@/stores/robot-stats"
import { readRobotStats, useRobotStatsEntry } from "@/stores/robot-stats"

export interface RobotStatsView extends RobotStatsEntry {
  readonly refresh: () => void
}

/**
 * The Safe Push statistics of the selected repository, read while `visible`: when the view shows and when the window
 * gets the focus (`readRobotStats`). A hidden view reads nothing.
 */
export function useRobotStats(visible: boolean): RobotStatsView {
  const repositoryPath = useSelectedProjectPath() ?? null
  const branchPrefix = useEffectiveBranchPrefix()
  const baselineBranch = useBranchSync(state => state.baselineBranch)
  const subject: RobotStatsSubject | null = repositoryPath === null || baselineBranch === null || branchPrefix === "" ? null : { repositoryPath, baselineBranch, branchPrefix }
  const entry = useRobotStatsEntry(repositoryPath)
  useEffect(() => {
    if (visible && repositoryPath !== null && baselineBranch !== null && branchPrefix !== "") {
      readRobotStats({ repositoryPath, baselineBranch, branchPrefix }, false)
    }
  }, [visible, repositoryPath, baselineBranch, branchPrefix])
  useWindowFocus(() => {
    if (visible && subject !== null) {
      readRobotStats(subject, false)
    }
  })
  return {
    ...entry,
    refresh: () => {
      if (subject !== null) {
        readRobotStats(subject, true)
      }
    },
  }
}
