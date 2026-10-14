import { toast } from "@/components/ui/toast"
import { useTauriListen } from "@/hooks/useTauriListen"
import { commands } from "@/lib/bindings"
import type { AgentCliInstall } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"

/** The toast text of an install: where agents find bd, and how to run it in a terminal when it is not on PATH */
export function describeInstall({ bd, on_path, skills }: AgentCliInstall): string {
  const agents = `Coding agents find bd through the skill${skills.length === 1 ? "" : "s"} in ${skills.join(", ")}.`
  if (on_path) {
    return `${agents} In a terminal, run bd.`
  }
  const directory = bd.slice(0, Math.max(bd.lastIndexOf("/"), bd.lastIndexOf("\\")))
  return `${agents} In a terminal, run ${bd}, or add ${directory} to PATH to run bd.`
}

async function installAgentCli(): Promise<void> {
  const result = await commands.installAgentCli()
  if (result.status === "error") {
    notifyError("Can't install the agent CLI", result.error)
    return
  }
  toast.add({ type: "success", title: "Agent CLI installed", description: describeInstall(result.data) })
}

/** Main window: the "Install Agent CLI…" menu item, `bd install` (docs/guidelines/agent-cli.md) */
export function useInstallAgentCli(): void {
  useTauriListen("install_agent_cli", () => {
    void installAgentCli()
  })
}
