import { PushSizeLimitSetting } from "@/components/settings/PushSizeLimitSetting"
import { SettingsSection, SettingSwitch } from "@/components/settings/SettingRow"
import { InlineCode } from "@/components/shared/InlineCode"
import { CONFIG_KEYS } from "@/lib/repository-config"
import { useSelectedProjectPath } from "@/stores/repository"

/**
 * Settings → Patronus (spec/repository/settings.spec.md#patronus): how Branch Deck and bd read the robots, and the size
 * limit of every push, the way of a branch to its Safe Push
 */
export function PatronusPage() {
  const path = useSelectedProjectPath()
  return (
    <>
      <SettingsSection title="Robots" description="How Branch Deck and bd read the robots of Patronus">
        <SettingSwitch
          name="patronusNewApi"
          label="New Patronus API"
          description="The progress of a running robot in one request and its merge profile in the robot list; the old requests while Patronus has them off"
        />
      </SettingsSection>
      <SettingsSection
        title="Push"
        description={
          <>
            {"Every push of the app and of bd: one that adds more files is refused before it is sent · "}
            <InlineCode>{CONFIG_KEYS.pushSizeLimit}</InlineCode>
          </>
        }
      >
        {/* one read per repository; without one, the global config alone */}
        <PushSizeLimitSetting key={path ?? ""} path={path ?? ""} />
      </SettingsSection>
    </>
  )
}
