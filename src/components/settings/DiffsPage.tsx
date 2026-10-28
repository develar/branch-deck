import { useId } from "react"
import type { ReactNode } from "react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ConfigReadError, SettingRow, SettingsSection } from "@/components/settings/SettingRow"
import { DiffLayoutButtons, WhitespaceMenu } from "@/components/shared/DiffViewToggle"
import { InlineCode } from "@/components/shared/InlineCode"
import { useConfigWrite } from "@/hooks/useConfigWrite"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import type { RepositoryConfigChange, WriteScope } from "@/lib/bindings"
import { CONFIG_KEYS, describeEveryRepositorySource, describeFallback, overriddenReason, repositoryName, resetLabel, writeErrorTitle } from "@/lib/repository-config"
import { useSelectedProjectPath } from "@/stores/repository"
import { setDiffLigatures, setDiffSoftWraps, useDiffLigatures, useDiffSoftWraps } from "@/stores/settings/diff-view-settings"

/**
 * Settings → Diffs & Merge: how every diff of the app shows its lines and what it ignores; how a sync merges a commit
 * that git stops at
 */
export function DiffsPage() {
  const path = useSelectedProjectPath()
  return (
    <>
      <SettingsSection title="Diffs" description="Every diff of the app: the commit preview, the reviews, the conflicts">
        <DiffSettings />
      </SettingsSection>
      <SettingsSection title="Merge" description="A conflict of git in a sync, in Update Project and in bd update">
        {/* one read per repository; without one, the global config alone */}
        <ResolveByWordsSetting key={path ?? ""} path={path ?? ""} />
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

/**
 * "Resolve conflicts by words" (`branchdeck.resolveByWords`, docs/design/git-operations.md "Resolve by words"): one
 * value for every repository in the global git config, which bd reads too. A repository whose `.git/config` sets the
 * key keeps its own value: the switch changes it there, and "Use Global" or "Use Default" removes it. The worktree
 * config and the command line win over both, so the switch is disabled then, with that reason.
 */
function ResolveByWordsSetting({ path }: { path: string }) {
  const { state, retry, showWritten } = useRepositoryConfig(path)
  const config = useConfigWrite(path, showWritten)
  const writing = config.writing
  const setting = state.status === "loaded" ? state.config.resolveByWords : null
  const name = repositoryName(path)
  const overridden = setting === null ? null : overriddenReason(setting)

  const write = (value: boolean | null, scope: WriteScope) => {
    const change: RepositoryConfigChange = { key: "resolveByWords", value }
    void config.write(change, scope, scope === "global" ? "Cannot change how every repository resolves conflicts by words" : writeErrorTitle(change, false, path))
  }

  return (
    <div className="flex flex-col gap-1">
      <SettingRow
        testId="setting-resolveByWords"
        label="Resolve conflicts by words"
        description={
          <>
            {"Commit a conflict that its words resolve · "}
            <InlineCode>{CONFIG_KEYS.resolveByWords}</InlineCode>
          </>
        }
        checked={setting?.value ?? true}
        disabled={setting === null || overridden !== null || writing}
        onCheckedChange={checked => write(checked, setting?.localSet === true ? "local" : "global")}
      />
      <div className="flex min-h-6 flex-col justify-center gap-0.5 pl-9 text-xs text-muted-foreground">
        {state.status === "error" && <ConfigReadError testId="setting-resolveByWords-error" message={state.message} onRetry={retry} />}
        {setting !== null && overridden === null && (
          <p className="flex items-center gap-2" data-testid="setting-resolveByWords-source">
            {describeEveryRepositorySource(setting, name, "On")}
            {setting.localSet && (
              <Tooltip>
                <TooltipTrigger render={<Button variant="ghost" size="xs" disabled={writing} onClick={() => write(null, "local")} />}>{resetLabel(setting)}</TooltipTrigger>
                <TooltipContent>{`Removes it from .git/config: back to ${describeFallback(setting, onOff(setting.fallbackValue))}`}</TooltipContent>
              </Tooltip>
            )}
          </p>
        )}
        {overridden !== null && (
          <p className="text-warning" data-testid="setting-resolveByWords-reason">
            {overridden}
          </p>
        )}
        {setting?.invalid != null && <p className="text-warning">{setting.invalid}</p>}
      </div>
    </div>
  )
}
