import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { AgentCliStatus } from "@/lib/bindings"
import { commands } from "@/lib/bindings"

/** Whether coding agents find bd through its skill (`agent_cli_status`); null until the first read answered */
const agentCliStore = createStore<{ status: AgentCliStatus | null }>(() => ({ status: null }))

/** Reads the status again: when a surface that shows it mounts, on a focus of its window, and after an install */
export function refreshAgentCliStatus(): void {
  void commands.agentCliStatus().then(status => agentCliStore.setState({ status }))
}

export function useAgentCliStatusValue(): AgentCliStatus | null {
  return useStore(agentCliStore, state => state.status)
}
