import { useAppCommand } from "@/hooks/useAppCommand"
import { commands } from "@/lib/bindings"
import type { AgentCliInstall, AgentRules } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { resolveHomeDir, tildePath } from "@/lib/home-path"
import { openSettings } from "@/lib/settings-window"
import { showToast } from "@/lib/toast-message"

/**
 * The toast text of an install: where agents find bd, how to run it in a terminal when it is not on PATH, and how agents
 * work from now on (Settings → Agents); paths under the home directory as `~`
 */
export function describeInstall({ bd, on_path, skills, agents }: AgentCliInstall, homeDir: string): string {
  const found = `Coding agents find bd through the skill${skills.length === 1 ? "" : "s"} in ${skills.map(it => tildePath(it, homeDir)).join(", ")}.`
  const directory = bd.slice(0, Math.max(bd.lastIndexOf("/"), bd.lastIndexOf("\\")))
  const terminal = on_path ? "In a terminal, run bd." : `In a terminal, run ${tildePath(bd, homeDir)}, or add ${tildePath(directory, homeDir)} to PATH to run bd.`
  return `${found} ${terminal} ${describeAgentRules(agents)}`
}

/** How agents work by the rules of every repository: "Agents work in their own worktree …; commit subjects stay as written." */
export function describeAgentRules({ lease, subject_branch }: AgentRules): string {
  const where = lease ? "Agents work in their own worktree and land each commit on your branch" : "Agents work in your checkout"
  return subject_branch ? `${where}, and name the branch in each commit subject.` : `${where}; commit subjects stay as written.`
}

/**
 * `bd install`, then a toast that says what it installed; from the main window it opens Settings → Agents, where the
 * rules change (`openAgentSettings`)
 */
export async function installAgentCli(openAgentSettings = true): Promise<AgentCliInstall | null> {
  const result = await commands.installAgentCli()
  if (result.status === "error") {
    notifyError("Cannot install the Agent CLI", result.error)
    return null
  }
  const description = describeInstall(result.data, await resolveHomeDir())
  showToast({
    title: "Agent CLI installed",
    description,
    ...(openAgentSettings && { action: { children: "Open Agent Settings", onClick: () => openSettings({ page: "agents" }) } }),
  })
  return result.data
}

/** Main window: the "Install Agent CLI…" menu item, `bd install` (docs/guidelines/agent-cli.md) */
export function useInstallAgentCli(): void {
  useAppCommand("install_agent_cli", () => {
    void installAgentCli()
  })
}
