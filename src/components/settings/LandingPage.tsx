import { Button } from "@/components/ui/button"
import { CompanionsSetting } from "@/components/settings/CompanionsSetting"
import { ConfigChoices, ConfigTextField, ScopedConfigSections } from "@/components/settings/ConfigControls"
import { ScopedConfigRow } from "@/components/settings/ScopedConfigRow"
import { SettingsSection, SettingSwitch } from "@/components/settings/SettingRow"
import { useConfigDraft } from "@/hooks/useConfigDraft"
import type { ScopedConfig } from "@/hooks/useScopedConfig"
import { useSettingsScope } from "@/hooks/useSettingsScope"
import type { LandingDefault, LandingSetting, RepositoryConfig } from "@/lib/bindings"
import {
  DEFAULT_PUSH_SIZE_LIMIT,
  LANDING_ROUTES,
  describeDefaultLanding,
  describeDetectedGitHubRepository,
  detectedGitHubRepositoryText,
  everyRepositoryValue,
  invalidGitHubRepositoryReason,
  invalidPushSizeLimitReason,
  isValidGitHubRepository,
  isValidPushSizeLimit,
  landingRouteLabel,
} from "@/lib/repository-config"

const ROUTE_CHOICES = LANDING_ROUTES.map(({ setting, label }) => ({ value: setting, label }))

/**
 * Settings → Landing (spec/repository/settings.spec.md#landing): how the work lands into master. A repository has its
 * route ("Land by", `branchdeck.landing`), the GitHub repository of its pull requests and its companions, which land in
 * an order with it; the push size limit is a value of every repository that one may override; the API of Patronus a
 * setting of the app, on "All repositories"
 */
export function LandingPage() {
  const { scope, setScope, path, name } = useSettingsScope()
  return (
    <ScopedConfigSections>
      {(config, scoped) => (
        <>
          <SettingsSection title="Route into master" description="How the work of the repository lands into master, in the app and brd">
            {scope === "all" ? (
              <p className="flex flex-wrap items-center gap-x-1 text-xs text-muted-foreground" data-testid="landing-per-repository">
                {path === ""
                  ? "Select a repository to set its route into master, its GitHub repository and its companions"
                  : "The route into master, the GitHub repository and the companions are set for each repository"}
                {path !== "" && (
                  <Button variant="link" size="xs" className="px-1" onClick={() => setScope("repository")}>
                    {`Open ${name}`}
                  </Button>
                )}
              </p>
            ) : (
              <>
                <LandingRouteRow config={config} scoped={scoped} />
                <GitHubRepositoryRow config={config} scoped={scoped} />
              </>
            )}
          </SettingsSection>
          {scope === "repository" && config !== null && (
            <SettingsSection title="Companions" description="Repositories whose branches land in an order with this one, in the app and brd">
              <CompanionsSetting path={path} config={config} showWritten={scoped.read.showWritten} />
            </SettingsSection>
          )}
          <SettingsSection title="Push" description="A push of the app or brd that adds more files is refused before it is sent">
            <PushSizeLimitRow config={config} scoped={scoped} />
          </SettingsSection>
          {scope === "all" && (
            <SettingsSection title="Safe Push" description="How Branch Deck and brd read the robots of Patronus">
              <SettingSwitch
                name="patronusNewApi"
                label="New Patronus API"
                description="The progress of a running robot in one request and its merge profile in the robot list; the old requests while Patronus has them off"
              />
            </SettingsSection>
          )}
        </>
      )}
    </ScopedConfigSections>
  )
}

/** The route an unset `branchdeck.landing` means, with why, as a line names it */
function defaultRouteText(landingDefault: LandingDefault): string {
  return `${landingRouteLabel(landingDefault.route)}, the default ${landingDefault.onSpace ? "on Space" : "off Space"}`
}

/**
 * "Land by": Safe Push · Push · Pull Request, the route in effect pressed (the default of the host of `origin` while no
 * config names it, which the line under it says; a click on it names it), written to `.git/config` only. The main window
 * syncs after a write (`repository_config_changed`), so its rows take the steps of the route
 */
function LandingRouteRow({ config, scoped }: { config: RepositoryConfig | null; scoped: ScopedConfig }) {
  const landingDefault = config?.landingDefault ?? null
  const route = config === null ? null : (config.landing.value ?? config.landingDefault.route)
  return (
    <ScopedConfigRow<LandingSetting | null>
      configKey="landing"
      label="Land by"
      description={LANDING_ROUTES.find(it => it.setting === route)?.description ?? ""}
      setting={config?.landing ?? null}
      words={{
        valueText: value => (value === null ? (landingDefault === null ? "" : defaultRouteText(landingDefault)) : landingRouteLabel(value)),
        repositoryOnly: { defaultText: landingDefault === null ? null : describeDefaultLanding(landingDefault) },
      }}
      busy={scoped.writing}
      onReset={() => void scoped.write({ key: "landing", value: null })}
      focusKey="landing"
      control={({ disabled, labelId }) => (
        <ConfigChoices<LandingSetting>
          choices={ROUTE_CHOICES}
          value={route}
          disabled={disabled}
          labelId={labelId}
          // the default pressed while unset: a click names it in the config
          namesPressed={config?.landing.localSet !== true}
          onChange={value => void scoped.write({ key: "landing", value })}
        />
      )}
    />
  )
}

/** "GitHub repository" (`owner/name`): where pull requests are opened; the detected one while no config names it */
function GitHubRepositoryRow({ config, scoped }: { config: RepositoryConfig | null; scoped: ScopedConfig }) {
  const { path } = useSettingsScope()
  const setting = config?.githubRepository ?? null
  const detected = config?.detectedGithubRepository ?? null
  const draft = useConfigDraft({
    path,
    shown: setting?.localSet === true ? (setting.value ?? "") : "",
    normalize: typing => typing.trim(),
    invalidReason: value => (isValidGitHubRepository(value) ? null : invalidGitHubRepositoryReason(value)),
    write: value => scoped.write({ key: "githubRepository", value }),
  })
  return (
    <ScopedConfigRow<string | null>
      configKey="githubRepository"
      label="GitHub repository"
      description="Where pull requests are opened"
      setting={setting}
      words={{
        valueText: value => value ?? detectedGitHubRepositoryText(detected),
        repositoryOnly: { defaultText: config === null ? null : describeDetectedGitHubRepository(detected) },
      }}
      busy={scoped.writing}
      onReset={() => void scoped.write({ key: "githubRepository", value: null })}
      error={draft.error}
      control={({ fieldDisabled }) => (
        <ConfigTextField
          draft={draft}
          label="GitHub repository"
          placeholder={(setting?.localSet === true ? null : setting?.value) ?? detected?.fullName ?? "owner/name"}
          disabled={fieldDisabled}
          focusKey="githubRepository"
        />
      )}
    />
  )
}

/**
 * "Push size limit" (`branchdeck.pushSizeLimit`, docs/design/git-operations.md "Push size"): a push that adds more
 * files is refused before it is sent; a size, 0 or off, refused in place as the backend would (`isValidPushSizeLimit`).
 * A write syncs nothing: a push reads the limit as it starts
 */
function PushSizeLimitRow({ config, scoped }: { config: RepositoryConfig | null; scoped: ScopedConfig }) {
  const { scope, path } = useSettingsScope()
  const setting = config?.pushSizeLimit ?? null
  const every = setting === null ? null : everyRepositoryValue(setting).value
  const shown = setting === null ? "" : scope === "all" ? (every ?? "") : setting.localSet ? (setting.value ?? "") : ""
  const draft = useConfigDraft({
    path,
    shown,
    normalize: typing => typing.trim(),
    invalidReason: value => (isValidPushSizeLimit(value) ? null : invalidPushSizeLimitReason(value)),
    write: value => scoped.write({ key: "pushSizeLimit", value }),
  })
  return (
    <ScopedConfigRow<string | null>
      configKey="pushSizeLimit"
      label="Push size limit"
      description="A size such as 50M or 1G, 0 or off for none"
      setting={setting}
      words={{ valueText: value => value ?? DEFAULT_PUSH_SIZE_LIMIT }}
      busy={scoped.writing}
      onReset={() => void scoped.write({ key: "pushSizeLimit", value: null })}
      error={draft.error}
      control={({ fieldDisabled }) => (
        <ConfigTextField
          draft={draft}
          label="Push size limit"
          placeholder={scope === "all" ? DEFAULT_PUSH_SIZE_LIMIT : (every ?? DEFAULT_PUSH_SIZE_LIMIT)}
          disabled={fieldDisabled}
          className="w-32"
        />
      )}
    />
  )
}
