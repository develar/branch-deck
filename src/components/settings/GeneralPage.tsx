import { SettingRow, SettingsSection, SettingSwitch } from "@/components/settings/SettingRow"
import { changeAIMode } from "@/hooks/useAIToggle"
import { FROM_MENU } from "@/lib/menu"
import { unattendedModeItem } from "@/lib/unattended"
import { aiConsentNote, useAISettings } from "@/stores/settings/aiSettings"
import { useAppSettings } from "@/stores/settings/appSettings"

/** Settings → General: Auto-sync on Focus, the branch names of Claude, Unattended Mode; the branch prefix is on Branches */
export function GeneralPage() {
  return (
    <>
      <SettingsSection title="Sync" description="When Branch Deck syncs by itself">
        <SettingSwitch name="autoSyncOnFocus" label="Auto-sync on Focus" description="Sync when the window gets the focus and a commit, a fetch or a push changed the branches" />
      </SettingsSection>
      <SettingsSection title="AI" description="Branch names for the selected commits in Move to…">
        <AISwitch />
      </SettingsSection>
      <SettingsSection title="Coding agents" description="While you leave them alone">
        <UnattendedSwitch />
      </SettingsSection>
    </>
  )
}

/**
 * Unattended Mode (openspec/specs/repository/unattended-mode/spec.md), the item of the palette and of the mark of the title
 * bar: the switch runs it, with no list of Denied Requests here (the toast of a switch off tells the count)
 */
function UnattendedSwitch() {
  const on = useAppSettings(state => state.unattended)
  const item = unattendedModeItem(on, false)
  return <SettingRow testId="setting-unattended" label="Unattended Mode" description={item.description} checked={on} onCheckedChange={() => item.onSelect(FROM_MENU)} />
}

/** On is the consent of the prompt in Move to…; off is its "Not now", and so is no answer yet */
function AISwitch() {
  const checked = useAISettings(state => state.aiMode === "claude")
  return (
    <SettingRow
      testId="setting-aiMode"
      label="Suggest branch names"
      description={aiConsentNote}
      checked={checked}
      onCheckedChange={value => changeAIMode(value ? "claude" : "disabled")}
    />
  )
}
