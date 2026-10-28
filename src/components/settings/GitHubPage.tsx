import { GitHubSettings } from "@/components/settings/GitHubSettings"
import { SettingsSection } from "@/components/settings/SettingRow"

/**
 * Settings → GitHub: where the token comes from and the app's own sign-in, read on each open of the window and on a
 * sign-in or sign-out of any window (`useAuthStatusOnOpen` of the Settings page, `useAuthSync` of every window)
 */
export function GitHubPage() {
  return (
    <SettingsSection title="GitHub" description="Pull requests and the requests for your review">
      <GitHubSettings />
    </SettingsSection>
  )
}
