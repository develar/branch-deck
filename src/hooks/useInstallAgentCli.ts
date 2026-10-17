import { useAppCommand } from "@/hooks/useAppCommand"
import { commands } from "@/lib/bindings"
import type { AgentCliInstall } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { notifyError } from "@/lib/errors"
import { resolveHomeDir, tildePath } from "@/lib/home-path"
import { showToast } from "@/lib/toast-message"

/**
 * The toast text of an install: where agents find bd, and how to run it in a terminal when it is not on PATH; paths
 * under the home directory as `~`
 */
export function describeInstall({ bd, on_path, skills }: AgentCliInstall, homeDir: string): string {
  const agents = `Coding agents find bd through the skill${skills.length === 1 ? "" : "s"} in ${skills.map(it => tildePath(it, homeDir)).join(", ")}.`
  if (on_path) {
    return `${agents} In a terminal, run bd.`
  }
  const directory = bd.slice(0, Math.max(bd.lastIndexOf("/"), bd.lastIndexOf("\\")))
  return `${agents} In a terminal, run ${tildePath(bd, homeDir)}, or add ${tildePath(directory, homeDir)} to PATH to run bd.`
}

async function installAgentCli(): Promise<void> {
  const result = await commands.installAgentCli()
  if (result.status === "error") {
    notifyError("Cannot install the Agent CLI", result.error)
    return
  }
  const { bd } = result.data
  const description = describeInstall(result.data, await resolveHomeDir())
  // the installed command is what the toast is about: its full path goes into a terminal or a PATH entry
  showToast({ title: "Agent CLI installed", description, action: { children: "Copy Path", onClick: () => void copyText(bd) } })
}

/** Main window: the "Install Agent CLI…" menu item, `bd install` (docs/guidelines/agent-cli.md) */
export function useInstallAgentCli(): void {
  useAppCommand("install_agent_cli", () => {
    void installAgentCli()
  })
}
