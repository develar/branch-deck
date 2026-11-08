import { useAppCommand } from "@/hooks/useAppCommand"
import { commands } from "@/lib/bindings"
import type { AgentCliInstall, AgentRules } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { resolveHomeDir, tildePath } from "@/lib/home-path"
import { openSettings } from "@/lib/settings-window"
import { showToast } from "@/lib/toast-message"
import { refreshAgentCliStatus } from "@/stores/agent-cli"

/**
 * The toast text of an install: where agents find brd, how to run it in a terminal when it is not on PATH, and how agents
 * work from now on (Settings → Agents); paths under the home directory as `~`
 */
export function describeInstall({ bd: brd, on_path, skills, agents }: AgentCliInstall, homeDir: string): string {
  const found = `Coding agents find brd through the skill${skills.length === 1 ? "" : "s"} in ${skills.map(it => tildePath(it, homeDir)).join(", ")}.`
  const directory = brd.slice(0, Math.max(brd.lastIndexOf("/"), brd.lastIndexOf("\\")))
  const terminal = on_path ? "In a terminal, run brd." : `In a terminal, run ${tildePath(brd, homeDir)}, or add ${tildePath(directory, homeDir)} to PATH to run brd.`
  return `${found} ${terminal} ${describeAgentRules(agents)}`
}

/**
 * How agents work by the rules of every repository: "Agents work in their own worktree …; commit subjects stay as written.",
 * and with `pull_request` that they open the pull requests themselves
 */
export function describeAgentRules({ lease, subject_branch, pull_request }: AgentRules): string {
  const where = lease ? "Agents work in their own worktree and land each commit on your branch" : "Agents work in your checkout"
  const subjects = subject_branch ? `${where}, and name the branch in each commit subject.` : `${where}; commit subjects stay as written.`
  return pull_request ? `${subjects} They push each branch and open its pull request without asking.` : subjects
}

/**
 * `brd install`, then a toast that says what it installed; from the main window it opens Settings → Agents, where the
 * rules change (`openAgentSettings`)
 */
export async function installAgentCli(openAgentSettings = true): Promise<AgentCliInstall | null> {
  const result = await commands.installAgentCli()
  if (result.status === "error") {
    notifyError("Cannot install the Agent CLI", result.error)
    return null
  }
  refreshAgentCliStatus()
  const description = describeInstall(result.data, await resolveHomeDir())
  showToast({
    title: "Agent CLI installed",
    description,
    ...(openAgentSettings && { action: { children: "Open Agent Settings", onClick: () => openSettings({ page: "agents" }) } }),
  })
  return result.data
}

/** Main window: the "Install Agent CLI…" menu item, `brd install` (docs/contributing/agent-cli.md) */
export function useInstallAgentCli(): void {
  useAppCommand("install_agent_cli", () => {
    void installAgentCli()
  })
}
