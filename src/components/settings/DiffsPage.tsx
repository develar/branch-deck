import { useId } from "react"
import type { ReactNode } from "react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ConfigReadError, ConfigSwitchRow, ConfigWarnings, SettingRow, SettingsSection } from "@/components/settings/SettingRow"
import { DiffLayoutButtons, WhitespaceMenu } from "@/components/shared/DiffViewToggle"
import { useConfigWrite } from "@/hooks/useConfigWrite"
import type { ConfigWrite } from "@/hooks/useConfigWrite"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import type { RepositoryConfigRead } from "@/hooks/useRepositoryConfig"
import type { RepositoryConfigChange, WriteScope } from "@/lib/bindings"
import { describeEveryRepositorySource, describeFallback, overriddenReason, repositoryName, resetLabel, writeErrorTitle } from "@/lib/repository-config"
import { useSelectedProjectPath } from "@/stores/repository"
import { setDiffLigatures, setDiffSoftWraps, useDiffLigatures, useDiffSoftWraps } from "@/stores/settings/diff-view-settings"

/**
 * Settings → Diffs & Merge: how every diff of the app shows its lines and what it ignores; how a sync merges a commit
 * that git stops at; whether the review window of a branch shows the review plan
 */
export function DiffsPage() {
  const path = useSelectedProjectPath()
  return (
    <>
      <SettingsSection title="Diffs" description="Every diff of the app: the commit preview, the reviews, the conflicts">
        <DiffSettings />
      </SettingsSection>
      {/* one read per repository; without one, the global config alone */}
      <EveryRepositorySettings key={path ?? ""} path={path ?? ""} />
    </>
  )
}

/** The sections whose switches give every repository one value of the global git config, over one read of it */
function EveryRepositorySettings({ path }: { path: string }) {
  const read = useRepositoryConfig(path)
  const config = useConfigWrite(path, read.showWritten)
  return (
    <>
      <SettingsSection title="Merge" description="A conflict of git in a sync, in Update Project and in bd update">
        {/* docs/design/git-operations.md "Resolve by words" */}
        <EveryRepositorySwitch
          path={path}
          read={read}
          config={config}
          configKey="resolveByWords"
          label="Resolve conflicts by words"
          description="Commit a conflict that its words resolve"
          globalErrorTitle="Cannot change how every repository resolves conflicts by words"
        />
      </SettingsSection>
      <SettingsSection title="Review" description="The review window of a branch">
        {/* spec/branch/review-plan.spec.md, Status: Experimental */}
        <EveryRepositorySwitch
          path={path}
          read={read}
          config={config}
          configKey="reviewExperimental"
          label="Review plan (experimental)"
          description="Mark Reviewed, Since Your Review, the Review Summary card and the badges of the files in the branch review"
          globalErrorTitle="Cannot change whether every repository shows the review plan"
        />
      </SettingsSection>
    </>
  )
}

/** The layout and the Whitespace menu, the controls above every diff too, then soft wraps and ligatures */
function DiffSettings() {
  const softWraps = useDiffSoftWraps()
  const ligatures = useDiffLigatures()
  return (
    <div className="flex flex-col gap-3">
      <ControlRow testId="setting-diffViewMode" label="Layout">
        <DiffLayoutButtons />
      </ControlRow>
      <ControlRow testId="setting-whitespacePolicy" label="Whitespace">
        <WhitespaceMenu named />
      </ControlRow>
      <SettingRow
        testId="setting-softWraps"
        label="Soft wraps"
        description="Break a long line at the edge of the diff; without wraps it scrolls"
        checked={softWraps}
        onCheckedChange={setDiffSoftWraps}
      />
      <SettingRow
        testId="setting-ligatures"
        label="Ligatures"
        description="Draw the ligatures of JetBrains Mono; each keeps the width of its characters"
        checked={ligatures}
        onCheckedChange={setDiffLigatures}
      />
    </div>
  )
}

/** A control that is no switch, after its label */
function ControlRow({ testId, label, children }: { testId: string; label: string; children: ReactNode }) {
  const labelId = useId()
  return (
    <div className="flex min-h-7 items-center gap-3" data-testid={testId}>
      <span id={labelId} className="w-24 shrink-0 text-sm">
        {label}
      </span>
      <div role="group" aria-labelledby={labelId}>
        {children}
      </div>
    </div>
  )
}

/** What the switch gives once the local value is gone, for the tooltip of its reset */
function onOff(value: boolean): string {
  return value ? "On" : "Off"
}

/** A switch of the global git config (`resolveByWords`, on by default; `reviewExperimental`, off by default) */
type EveryRepositoryKey = "resolveByWords" | "reviewExperimental"

interface EveryRepositorySwitchProps {
  path: string
  read: RepositoryConfigRead
  config: ConfigWrite
  configKey: EveryRepositoryKey
  label: string
  description: string
  /** The title of the toast when the write of the global config fails */
  globalErrorTitle: string
}

/**
 * A switch with one value for every repository in the global git config, which bd reads too, of the default Rust gives. A
 * repository whose `.git/config` sets the key keeps its own value: the switch changes it there, and "Use Global" or
 * "Use Default" removes it. The worktree config and the command line win over both, so the switch is disabled then,
 * with that reason.
 */
function EveryRepositorySwitch({ path, read, config, configKey, label, description, globalErrorTitle }: EveryRepositorySwitchProps) {
  const { state, retry } = read
  const writing = config.writing
  const setting = state.status === "loaded" ? state.config[configKey] : null
  const name = repositoryName(path)
  const overridden = setting === null ? null : overriddenReason(setting)

  const write = (value: boolean | null, scope: WriteScope) => {
    const change: RepositoryConfigChange = { key: configKey, value }
    void config.write(change, scope, scope === "global" ? globalErrorTitle : writeErrorTitle(change, false, path))
  }

  return (
    <div className="flex flex-col gap-1">
      <ConfigSwitchRow
        configKey={configKey}
        label={label}
        description={description}
        checked={setting?.value ?? true}
        disabled={setting === null || overridden !== null || writing}
        onCheckedChange={checked => write(checked, setting?.localSet === true ? "local" : "global")}
      />
      <div className="flex min-h-6 flex-col justify-center gap-0.5 pl-9 text-xs text-muted-foreground">
        {state.status === "error" && <ConfigReadError testId={`setting-${configKey}-error`} message={state.message} onRetry={retry} />}
        {setting !== null && overridden === null && (
          <p className="flex items-center gap-2" data-testid={`setting-${configKey}-source`}>
            {describeEveryRepositorySource(setting, name, "On")}
            {setting.localSet && (
              <Tooltip>
                <TooltipTrigger render={<Button variant="ghost" size="xs" disabled={writing} onClick={() => write(null, "local")} />}>{resetLabel(setting)}</TooltipTrigger>
                <TooltipContent>{`Removes it from .git/config: back to ${describeFallback(setting, onOff(setting.fallbackValue))}`}</TooltipContent>
              </Tooltip>
            )}
          </p>
        )}
        <ConfigWarnings configKey={configKey} overridden={overridden} invalid={setting?.invalid} />
      </div>
    </div>
  )
}
