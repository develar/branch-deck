import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { AgentCliStatus } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { logWarning } from "@/lib/errors"

/** Whether coding agents find bd through its skill (`agent_cli_status`); null until the first read answered */
const agentCliStore = createStore<{ status: AgentCliStatus | null }>(() => ({ status: null }))

/** Reads the status again: when a surface that shows it mounts, on a focus of its window, and after an install */
export function refreshAgentCliStatus(): void {
  commands
    .agentCliStatus()
    .then(status => agentCliStore.setState({ status }))
    .catch((error: unknown) => logWarning("Cannot read whether coding agents find bd", error))
}

export function useAgentCliStatusValue(): AgentCliStatus | null {
  return useStore(agentCliStore, state => state.status)
}
