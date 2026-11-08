import { BranchPrefixSetting } from "@/components/settings/BranchPrefixSetting"
import { ConfigChoices, ConfigSwitch, ConfigTextField, ScopedConfigSections } from "@/components/settings/ConfigControls"
import { ScopedConfigRow } from "@/components/settings/ScopedConfigRow"
import { SettingsSection } from "@/components/settings/SettingRow"
import { useConfigDraft } from "@/hooks/useConfigDraft"
import type { ScopedConfig } from "@/hooks/useScopedConfig"
import { useSettingsScope } from "@/hooks/useSettingsScope"
import type { RepositoryConfig, SymbolMode } from "@/lib/bindings"
import { INVALID_PREFIX_TEXT, isValidBranchPrefix, normalizeBranchPrefix } from "@/lib/branch-prefix"
import { SYMBOL_MODES, symbolModeLabel, withAppPrefix } from "@/lib/repository-config"
import { useAppSettings } from "@/stores/settings/appSettings"

const SYMBOL_CHOICES = SYMBOL_MODES.map(({ mode, label }) => ({ value: mode, label }))

/** "On" or "Off", as a line names a switch */
function onOff(value: boolean): string {
  return value ? "On" : "Off"
}

/**
 * Settings → Branches (spec/repository/settings.spec.md#branches): how the branches are named, how a sync groups and
 * merges the commits, whether the review window shows its plan. On "All repositories" the prefix of the app (which
 * names the branches of every repository without its own, `brd` too) and the global git config; on the repository's
 * tab its own prefix and `.git/config`
 */
export function BranchesPage() {
  const { scope } = useSettingsScope()
  return (
    <ScopedConfigSections>
      {(config, scoped) => (
        <>
          <SettingsSection title="Names" description="The prefix of your virtual branches">
            {scope === "all" ? <BranchPrefixSetting /> : <RepositoryPrefixRow config={config} scoped={scoped} />}
          </SettingsSection>
          <SettingsSection title="Sync" description="How a sync groups the commits, and merges a conflict of git">
            <ScopedConfigRow
              configKey="symbols"
              label="Symbol dependencies"
              description={SYMBOL_MODES.find(it => it.mode === (config === null ? null : scope === "all" ? config.symbols.fallbackValue : config.symbols.value))?.description ?? ""}
              setting={config?.symbols ?? null}
              words={{ valueText: symbolModeLabel }}
              busy={scoped.writing}
              onReset={() => void scoped.write({ key: "symbols", value: null })}
              focusKey="symbols"
              control={({ value, disabled, labelId }) => (
                <ConfigChoices<SymbolMode>
                  choices={SYMBOL_CHOICES}
                  value={value}
                  disabled={disabled}
                  labelId={labelId}
                  namesPressed={scope === "repository" && config?.symbols.localSet !== true}
                  onChange={mode => void scoped.write({ key: "symbols", value: mode })}
                />
              )}
            />
            <ScopedConfigRow
              configKey="resolveByWords"
              label="Resolve conflicts by words"
              description="Commit a conflict that its words resolve, in a sync, Update Project and brd update"
              setting={config?.resolveByWords ?? null}
              words={{ valueText: onOff }}
              busy={scoped.writing}
              onReset={() => void scoped.write({ key: "resolveByWords", value: null })}
              focusKey="resolveByWords"
              control={({ value, disabled, labelId }) => (
                <ConfigSwitch value={value} disabled={disabled} labelId={labelId} onChange={on => void scoped.write({ key: "resolveByWords", value: on })} />
              )}
            />
          </SettingsSection>
          <SettingsSection title="Review">
            {/* spec/branch/review-plan.spec.md, Status: Experimental */}
            <ScopedConfigRow
              configKey="reviewExperimental"
              label="Review plan (experimental)"
              description="Mark Reviewed, Since Your Review and the plan of the files in the review window of a branch"
              setting={config?.reviewExperimental ?? null}
              words={{ valueText: onOff }}
              busy={scoped.writing}
              onReset={() => void scoped.write({ key: "reviewExperimental", value: null })}
              focusKey="reviewExperimental"
              control={({ value, disabled, labelId }) => (
                <ConfigSwitch value={value} disabled={disabled} labelId={labelId} onChange={on => void scoped.write({ key: "reviewExperimental", value: on })} />
              )}
            />
          </SettingsSection>
        </>
      )}
    </ScopedConfigSections>
  )
}

/**
 * The repository's own prefix (`branchdeck.branchPrefix` of `.git/config`), which names its branches instead of the
 * prefix of every repository (spec/repository/settings.spec.md#branch-prefix-order): the prefix of the app wins over a
 * global or system git one, so the row shows and falls back to it
 */
function RepositoryPrefixRow({ config, scoped }: { config: RepositoryConfig | null; scoped: ScopedConfig }) {
  const { path } = useSettingsScope()
  const appPrefix = normalizeBranchPrefix(useAppSettings(state => state.globalUserBranchPrefix))
  const setting = config === null ? null : withAppPrefix(config.branchPrefix, appPrefix)
  const draft = useConfigDraft({
    path,
    shown: setting?.localSet === true ? (setting.value ?? "") : "",
    normalize: normalizeBranchPrefix,
    invalidReason: value => (isValidBranchPrefix(value) ? null : INVALID_PREFIX_TEXT),
    write: value => scoped.write({ key: "branchPrefix", value }),
  })
  const inherited = setting?.fallbackValue ?? ""
  return (
    <ScopedConfigRow
      configKey="branchPrefix"
      label="Branch prefix"
      description="Used here instead of the prefix of all repositories"
      setting={setting}
      words={{ valueText: value => value ?? "no prefix" }}
      busy={scoped.writing}
      onReset={() => void scoped.write({ key: "branchPrefix", value: null })}
      error={draft.error}
      control={({ fieldDisabled }) => (
        <ConfigTextField draft={draft} label="Branch prefix" placeholder={inherited || "your-name"} disabled={fieldDisabled} focusKey="branchPrefix" />
      )}
    />
  )
}
