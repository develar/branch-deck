import { useDebouncedRead } from "@/hooks/useDebouncedRead"
import type { DebouncedRead } from "@/hooks/useDebouncedRead"
import type { CommitGroup } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { useBranchSync } from "@/stores/branch-sync"
import { useSelectedProjectPath } from "@/stores/repository"

/**
 * The group a typed message puts its commit into, as Rust groups it with the table of the selected repository
 * (`preview_group`: its subject, the stacked branches, the issue groups and the stack parents of the last grouping),
 * asked once the typing pauses and again after a sync; null `message` asks nothing. The answer is null for a commit the
 * message leaves unassigned, and for a failed read
 */
export function usePreviewGroup(message: string | null): DebouncedRead<CommitGroup | null> {
  const repositoryPath = useSelectedProjectPath() ?? ""
  const syncCount = useBranchSync(state => state.syncCount)
  const ask = async (value: string) => {
    const result = await invokeCommand(() => commands.previewGroup({ repositoryPath, message: value }))
    return result.status === "ok" ? result.data : null
  }
  return useDebouncedRead(message, ask, `${repositoryPath}\n${syncCount}`)
}
