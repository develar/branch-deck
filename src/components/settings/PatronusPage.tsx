import { SettingsSection, SettingSwitch } from "@/components/settings/SettingRow"

/** Settings → Patronus (spec/repository/settings.spec.md#patronus): how Branch Deck and bd read the robots */
export function PatronusPage() {
  return (
    <SettingsSection title="Robots" description="How Branch Deck and bd read the robots of Patronus">
      <SettingSwitch
        name="patronusNewApi"
        label="New Patronus API"
        description="The progress of a running robot in one request and its merge profile in the robot list; the old requests while Patronus has them off"
      />
    </SettingsSection>
  )
}
