import { useEffect } from "react"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import type { AgentCliStatus } from "@/lib/bindings"
import { refreshAgentCliStatus, useAgentCliStatusValue } from "@/stores/agent-cli"

/**
 * Whether coding agents find bd, read when the surface mounts (and again for each `readKey`, the opens of Settings), on
 * each focus of the window and after an install; null until the first read answered
 */
export function useAgentCliStatus(readKey = 0): AgentCliStatus | null {
  useEffect(() => {
    refreshAgentCliStatus()
  }, [readKey])
  useWindowFocus(refreshAgentCliStatus)
  return useAgentCliStatusValue()
}
