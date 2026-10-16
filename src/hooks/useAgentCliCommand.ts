import { useEffect, useState } from "react"
import type { AgentCliCommand } from "@/lib/bindings"
import { commands } from "@/lib/bindings"

/**
 * How this app's bd runs in the repository, read when the view opens: the copy must write the clipboard within the
 * click, so it can't wait for the backend. `undefined` while it is read, null when the app can't tell its binary
 */
export function useAgentCliCommand(repositoryPath: string | null): AgentCliCommand | null | undefined {
  const [bd, setBd] = useState<{ repositoryPath: string; command: AgentCliCommand | null } | null>(null)
  useEffect(() => {
    if (repositoryPath === null) {
      return
    }
    let current = true
    void commands.agentCliCommand(repositoryPath).then(result => {
      if (current) {
        setBd({ repositoryPath, command: result.status === "ok" ? result.data : null })
      }
    })
    return () => {
      current = false
    }
  }, [repositoryPath])
  return bd !== null && bd.repositoryPath === repositoryPath ? bd.command : undefined
}
