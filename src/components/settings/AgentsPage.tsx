import { useEffect, useId, useState } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { ConfigReadError, SettingRow, SettingsSection } from "@/components/settings/SettingRow"
import { CopyButton } from "@/components/shared/CopyButton"
import { InlineCode } from "@/components/shared/InlineCode"
import { InstallAgentCliButton } from "@/components/shared/InstallAgentCliButton"
import { useAgentCliStatus } from "@/hooks/useAgentCliStatus"
import { useConfigWrite } from "@/hooks/useConfigWrite"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import { resolveHomeDir, tildePath } from "@/lib/home-path"
import { CONFIG_KEYS, describeGlobalSource, overriddenReason, repositoryName, writeErrorTitle } from "@/lib/repository-config"
import { openSettings } from "@/lib/settings-window"
import { useSelectedProjectPath } from "@/stores/repository"

/** The two rules of `bd workspace`: their key, what they do, their default and the title of a failed global write */
const AGENT_RULES = [
  {
    configKey: "agentLease",
    label: "Work in their own worktree",
    description: "Each agent leases a worktree and lands each commit on your branch",
    defaultValue: true,
    globalErrorTitle: "Cannot change where agents work in every repository",
  },
  {
    configKey: "agentSubjectBranch",
    label: "Name the branch in each commit subject",
    description: "(auth) Add login form: the commit goes into the branch auth",
    defaultValue: false,
    globalErrorTitle: "Cannot change how agents write commit subjects in every repository",
  },
] as const

type AgentRuleKey = (typeof AGENT_RULES)[number]["configKey"]

/**
 * Settings → Agents (spec/repository/settings.spec.md#agents): whether coding agents find bd through its skill, and how
 * they work, which `bd workspace` tells an agent before its first edit: one value for every repository in the global git
 * config, and the selected repository's own
 */
export function AgentsPage({ openCount }: { openCount: number }) {
  const path = useSelectedProjectPath() ?? ""
  return (
    <>
      <SettingsSection title="Coding agents" description="Agents such as Claude Code and Codex ask bd workspace before their first edit; what you tell an agent wins">
        <AgentSkill openCount={openCount} />
        {AGENT_RULES.map(rule => (
          // one read per repository; without one, the global config alone
          <AgentRuleSetting key={`${rule.configKey}:${path}`} path={path} {...rule} />
        ))}
        <p className="text-xs text-muted-foreground" data-testid="agent-tools">
          Agents also run teamcity and youtrack
          <Button variant="link" size="xs" className="px-1" onClick={() => openSettings({ page: "tools" })}>
            Open Tools
          </Button>
        </p>
      </SettingsSection>
    </>
  )
}

/** Whether the skill is installed (`agent_cli_status`, asked on each open and focus), else the install */
function AgentSkill({ openCount }: { openCount: number }) {
  const status = useAgentCliStatus(openCount)
  const [homeDir, setHomeDir] = useState("")
  useEffect(() => {
    void resolveHomeDir().then(setHomeDir)
  }, [])

  if (status === null) {
    return null
  }
  if (status.bd === null) {
    return (
      <div className="flex min-h-6 items-center gap-2 text-xs text-muted-foreground" data-testid="agent-skill">
        The agent skill is not installed
        <InstallAgentCliButton openAgentSettings={false} />
      </div>
    )
  }
  const bd = status.bd
  return (
    <div className="flex min-h-6 items-center gap-1 text-xs text-muted-foreground" data-testid="agent-skill">
      <span
        className="min-w-0 truncate"
        title={bd}
      >{`Installed: agents find bd (${tildePath(bd, homeDir)}) through ${status.skills.map(it => tildePath(it, homeDir)).join(", ")}`}</span>
      <CopyButton text={() => bd} tooltip="Copy the path of bd" alwaysVisible />
    </div>
  )
}

/**
 * One rule: the switch is its value for every repository (the global git config), the buttons the selected
 * repository's own (`.git/config`): "Like Global" removes it there. The worktree config and the command line win over
 * both, so the buttons are disabled then, with that reason
 */
function AgentRuleSetting({
  path,
  configKey,
  label,
  description,
  defaultValue,
  globalErrorTitle,
}: {
  path: string
  configKey: AgentRuleKey
  label: string
  description: string
  defaultValue: boolean
  globalErrorTitle: string
}) {
  const { state, retry, startWrite } = useRepositoryConfig(path)
  const { writing, write } = useConfigWrite(path, startWrite)
  const labelId = useId()
  const setting = state.status === "loaded" ? state.config[configKey] : null
  const overridden = setting === null ? null : overriddenReason(setting)
  // the value of every repository: the global or system one, else the default
  const everywhere = setting?.fallbackValue ?? defaultValue
  const own = setting?.scope === "local" ? setting.value : null
  const choices = [
    { label: "Like Global", value: null },
    { label: "On", value: true },
    { label: "Off", value: false },
  ] as const

  return (
    <div className="flex flex-col gap-1">
      <SettingRow
        testId={`setting-${configKey}`}
        label={label}
        description={
          <>
            {`${description} · `}
            <InlineCode>{CONFIG_KEYS[configKey]}</InlineCode>
          </>
        }
        checked={everywhere}
        disabled={setting === null || writing}
        onCheckedChange={checked => void write({ key: configKey, value: checked }, "global", globalErrorTitle)}
      />
      <div className="flex min-h-6 flex-col justify-center gap-1.5 pl-9 text-xs text-muted-foreground">
        {state.status === "error" && <ConfigReadError testId={`setting-${configKey}-error`} message={state.message} onRetry={retry} />}
        {setting !== null && <p data-testid={`setting-${configKey}-source`}>{describeGlobalSource(setting.fallbackScope, defaultValue ? "On" : "Off")}</p>}
        {setting !== null && path !== "" && (
          <div className="flex items-center gap-2" data-testid={`setting-${configKey}-repository`}>
            <span id={labelId}>{`In ${repositoryName(path)}`}</span>
            <FieldGroup role="group" aria-labelledby={labelId}>
              {choices.map(choice => {
                const pressed = overridden === null && own === choice.value
                return (
                  <Button
                    key={choice.label}
                    size="xs"
                    variant="outline"
                    aria-pressed={pressed}
                    className={cn(pressed && "border-primary/50 text-primary hover:text-primary")}
                    disabled={overridden !== null || writing}
                    onClick={() => {
                      if (!pressed) {
                        const change = { key: configKey, value: choice.value }
                        void write(change, "local", writeErrorTitle(change, false, path))
                      }
                    }}
                  >
                    {choice.label}
                  </Button>
                )
              })}
            </FieldGroup>
          </div>
        )}
        {overridden !== null && (
          <p className="text-warning" data-testid={`setting-${configKey}-reason`}>
            {overridden}
          </p>
        )}
        {setting?.invalid != null && <p className="text-warning">{setting.invalid}</p>}
      </div>
    </div>
  )
}
