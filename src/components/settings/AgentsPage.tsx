import { Button } from "@/components/ui/button"
import { ConfigSwitch, ScopedConfigSections } from "@/components/settings/ConfigControls"
import { ScopedConfigRow } from "@/components/settings/ScopedConfigRow"
import { SettingsSection } from "@/components/settings/SettingRow"
import { InstallAgentCliButton } from "@/components/shared/InstallAgentCliButton"
import { useAgentCliStatus } from "@/hooks/useAgentCliStatus"
import { useSettingsScope } from "@/hooks/useSettingsScope"
import { openSettings } from "@/lib/settings-window"

/** The three rules of `brd workspace`: their key, what they do */
const AGENT_RULES = [
  {
    configKey: "agentLease",
    label: "Work in its own worktree",
    description: "Each agent leases a worktree; its commits arrive on your branch one by one",
  },
  {
    configKey: "agentSubjectBranch",
    label: "Name the branch in each commit subject",
    description: "(auth) Add login form: the commit goes into the branch auth",
  },
  {
    configKey: "agentPullRequest",
    label: "Push the branch and open its pull request",
    description: "After each commit arrives on your branch, the agent pushes it and opens its pull request without asking",
  },
] as const

/** "On" or "Off", as a line names a switch */
function onOff(value: boolean): string {
  return value ? "On" : "Off"
}

/**
 * Settings → Agents (spec/repository/settings.spec.md#agents): how coding agents work, which `brd workspace` tells an
 * agent before its first edit, one value for every repository in the global git config and the repository's own. The
 * pull request after a commit is off where the repository lands by a push, as Rust says (`agentPullRequestOff`). While
 * the skill is not installed no agent asks: one line with the install. The skill itself is on Tools, beside the others
 */
export function AgentsPage({ openCount }: { openCount: number }) {
  const { scope, name } = useSettingsScope()
  const status = useAgentCliStatus(openCount)
  return (
    <>
      {status !== null && status.bd === null && (
        <p className="flex min-h-6 flex-wrap items-center gap-2 text-xs text-muted-foreground" data-testid="agent-skill">
          Agents read these rules once the skill is installed
          <InstallAgentCliButton openAgentSettings={false} />
        </p>
      )}
      <SettingsSection title="Coding agents" description="Agents such as Claude Code and Codex ask brd workspace before their first edit; what you tell an agent wins">
        <ScopedConfigSections>
          {(config, scoped) =>
            AGENT_RULES.map(rule => {
              const off = rule.configKey === "agentPullRequest" ? (config?.agentPullRequestOff ?? null) : null
              return (
                <ScopedConfigRow
                  key={rule.configKey}
                  configKey={rule.configKey}
                  label={rule.label}
                  description={rule.description}
                  setting={config?.[rule.configKey] ?? null}
                  words={{ valueText: onOff }}
                  busy={scoped.writing}
                  onReset={() => void scoped.write({ key: rule.configKey, value: null })}
                  focusKey={rule.configKey}
                  notApplied={
                    off === null ? null : (
                      <>
                        {scope === "all" ? `In ${name}: ${off}` : off}
                        <Button variant="link" size="xs" className="px-1" onClick={() => openSettings({ page: "landing", scope: "repository", focus: "landing" })}>
                          Change Route
                        </Button>
                      </>
                    )
                  }
                  control={({ value, disabled, labelId }) => (
                    <ConfigSwitch
                      value={off !== null && scope === "repository" ? false : value}
                      disabled={disabled || (off !== null && scope === "repository")}
                      labelId={labelId}
                      onChange={on => void scoped.write({ key: rule.configKey, value: on })}
                    />
                  )}
                />
              )
            })
          }
        </ScopedConfigSections>
      </SettingsSection>
    </>
  )
}
