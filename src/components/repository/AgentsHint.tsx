import { Button } from "@/components/ui/button"
import { InstallAgentCliButton } from "@/components/shared/InstallAgentCliButton"
import { useAgentCliStatus } from "@/hooks/useAgentCliStatus"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import { describeAgentsHere } from "@/lib/agents-hint"
import { openSettings } from "@/lib/settings-window"
import { useSelectedProjectPath } from "@/stores/repository"

/**
 * Where coding agents work beside the user, said on the way to the first branches (Ready to Sync, No active branches):
 * the user stays in this checkout, an agent leases a worktree of its own through bd (`branchdeck.agentLease` of the
 * repository, on by default) once the Agent CLI is installed. Its action installs it, then opens Settings → Agents
 */
export function AgentsHint() {
  const path = useSelectedProjectPath() ?? ""
  const { state } = useRepositoryConfig(path)
  const status = useAgentCliStatus()
  if (status === null || state.status !== "loaded") {
    return null
  }
  const installed = status.bd !== null
  return (
    <div className="flex flex-col items-start gap-1.5" data-testid="agents-hint">
      {/* a div: the alert of No active branches spaces its paragraphs */}
      <div>{describeAgentsHere(state.config.agentLease.value, installed)}</div>
      {installed ? (
        <Button size="xs" variant="outline" onClick={() => openSettings({ page: "agents" })}>
          Open Agent Settings
        </Button>
      ) : (
        <InstallAgentCliButton openAgentSettings />
      )}
    </div>
  )
}
