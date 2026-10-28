import { SettingsSection } from "@/components/settings/SettingRow"
import { YouTrackSettings } from "@/components/settings/YouTrackSettings"

/**
 * Settings → YouTrack: the sign-in, read on each open of the window and on a sign-in or sign-out of any window
 * (`useAuthStatusOnOpen` of the Settings page, `useAuthSync` of every window)
 */
export function YouTrackPage() {
  return (
    <SettingsSection title="YouTrack" description="Hover an issue key to see the issue, and mark it fixed once its work lands">
      <YouTrackSettings />
    </SettingsSection>
  )
}
