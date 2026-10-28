import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { ConfigRow } from "@/components/settings/ConfigRow"
import { PushSizeLimitSetting } from "@/components/settings/PushSizeLimitSetting"
import { ConfigReadError, SettingsSection, SettingSwitch } from "@/components/settings/SettingRow"
import { InlineCode } from "@/components/shared/InlineCode"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { useConfigWrite } from "@/hooks/useConfigWrite"
import type { RepositoryConfigRead } from "@/hooks/useRepositoryConfig"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import type { LandingSetting, RepositoryConfig } from "@/lib/bindings"
import { CONFIG_KEYS, LANDING_ROUTES, describeDefaultLanding, describeFallback, landingRouteLabel, overriddenReason, writeErrorTitle } from "@/lib/repository-config"
import { useSelectedProjectPath } from "@/stores/repository"

/**
 * Settings → Landing (spec/repository/settings.spec.md#landing): how the work of the selected repository lands into
 * master ("Land by", `branchdeck.landing`) with how Branch Deck and bd read the robots of Patronus while a Safe Push
 * lands it, then the size limit of every push
 */
export function LandingPage() {
  const path = useSelectedProjectPath()
  return path === undefined ? <LandingSections path="" read={null} /> : <RepositoryLanding key={path} path={path} />
}

/** The page of the repository at `path`: its settings read once for the route, which decides the sections shown */
function RepositoryLanding({ path }: { path: string }) {
  const read = useRepositoryConfig(path)
  return <LandingSections path={path} read={read} />
}

/** The effective route of `config`: the one it names, else the default of the host of `origin` */
function effectiveRoute(config: RepositoryConfig): LandingSetting {
  return config.landing.value ?? config.landingDefault.route
}

/**
 * The sections: the route of the repository (without one, only how the robots are read), the switch of the robots of
 * Patronus while a robot lands it, the push size limit always
 */
function LandingSections({ path, read }: { path: string; read: RepositoryConfigRead | null }) {
  const config = read?.state.status === "loaded" ? read.state.config : null
  // without a repository, or before its settings answer, the robots of Patronus stay offered
  const patronus = config === null || effectiveRoute(config) === "patronus"
  return (
    <>
      <SettingsSection title="Route into master" description="How the work of the repository lands into master, in the app and bd">
        {read?.state.status === "loading" && <LoadingLine>Reading the settings…</LoadingLine>}
        {read?.state.status === "error" && <ConfigReadError testId="setting-landing-error" message={read.state.message} onRetry={read.retry} />}
        {config !== null && read !== null && <LandingRouteRow path={path} config={config} showWritten={read.showWritten} />}
        {/* how the robots are read, while a robot lands the repository */}
        {patronus && (
          <SettingSwitch
            name="patronusNewApi"
            label="New Patronus API"
            description="The progress of a running robot in one request and its merge profile in the robot list; the old requests while Patronus has them off"
          />
        )}
      </SettingsSection>
      <SettingsSection
        title="Push"
        description={
          <>
            {"A push of the app or bd that adds more files is refused before it is sent · "}
            <InlineCode>{CONFIG_KEYS.pushSizeLimit}</InlineCode>
          </>
        }
      >
        {/* one read per repository; without one, the global config alone */}
        <PushSizeLimitSetting key={path} path={path} />
      </SettingsSection>
    </>
  )
}

/**
 * "Land by": Safe Push · Push · Pull Request, the effective route pressed (the default of the host of `origin` while no
 * config names it, which the line under it says), written to `.git/config` of the repository; "Use Default" removes it.
 * The main window syncs after a write (`repository_config_changed`), so its rows take the steps of the route
 */
function LandingRouteRow({ path, config, showWritten }: { path: string; config: RepositoryConfig; showWritten: RepositoryConfigRead["showWritten"] }) {
  const { write, writing } = useConfigWrite(path, showWritten)
  const setting = config.landing
  const route = effectiveRoute(config)
  const set = (value: LandingSetting | null) => void write({ key: "landing", value }, "local", writeErrorTitle({ key: "landing", value }, false, path))
  return (
    <ConfigRow
      configKey="landing"
      label="Land by"
      setting={setting}
      fallback={describeFallback(
        setting,
        landingRouteLabel(config.landingDefault.route),
        `${landingRouteLabel(config.landingDefault.route)}, the default ${config.landingDefault.onSpace ? "on Space" : "off Space"}`,
      )}
      busy={writing}
      onReset={() => set(null)}
      description={LANDING_ROUTES.find(it => it.setting === route)?.description ?? ""}
      unsetSource={describeDefaultLanding(config.landingDefault)}
      control={labelId => (
        <FieldGroup role="group" aria-labelledby={labelId}>
          {LANDING_ROUTES.map(({ setting: value, label }) => {
            const pressed = route === value
            return (
              <Button
                key={value}
                size="xs"
                variant="outline"
                aria-pressed={pressed}
                className={cn(pressed && "border-primary/50 text-primary hover:text-primary")}
                disabled={overriddenReason(setting) !== null || writing}
                onClick={() => {
                  // the default pressed while unset: a click names it in the config
                  if (!pressed || !setting.localSet) {
                    set(value)
                  }
                }}
              >
                {label}
              </Button>
            )
          })}
        </FieldGroup>
      )}
    />
  )
}
