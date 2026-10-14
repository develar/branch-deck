import { useTauriListen } from "@/hooks/useTauriListen"
import { commands } from "@/lib/bindings"
import type { AgentCliInstall } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { notifyError } from "@/lib/errors"
import { showToast } from "@/lib/toast-message"

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
    notifyError("Cannot install the Agent CLI", result.error)
    return
  }
  const { bd } = result.data
  // the installed command is what the toast is about: its path goes into a terminal or a PATH entry
  showToast({ title: "Agent CLI installed", description: describeInstall(result.data), action: { children: "Copy Path", onClick: () => void copyText(bd) } })
}

/** Main window: the "Install Agent CLI…" menu item, `bd install` (docs/guidelines/agent-cli.md) */
export function useInstallAgentCli(): void {
  useTauriListen("install_agent_cli", () => {
    void installAgentCli()
  })
}
