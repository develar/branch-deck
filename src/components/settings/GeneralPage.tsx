import { BranchPrefixSetting } from "@/components/settings/BranchPrefixSetting"
import { SettingRow, SettingsSection, SettingSwitch } from "@/components/settings/SettingRow"
import { changeAIMode } from "@/hooks/useAIToggle"
import { aiConsentNote, useAISettings } from "@/stores/settings/aiSettings"

/** Settings → General: the branch prefix, Auto-sync on Focus, the branch names of Claude */
export function GeneralPage() {
  return (
    <>
      <SettingsSection title="Branches" description="The prefix of your virtual branches, in every repository without one of its own">
        <BranchPrefixSetting />
      </SettingsSection>
      <SettingsSection title="Sync" description="When Branch Deck syncs by itself">
        <SettingSwitch name="autoSyncOnFocus" label="Auto-sync on Focus" description="Sync when the window gets the focus and a commit, a fetch or a push changed the branches" />
      </SettingsSection>
      <SettingsSection title="AI" description="Branch names for the selected commits in Move to…">
        <AISwitch />
      </SettingsSection>
    </>
  )
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
