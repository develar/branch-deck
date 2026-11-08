import { GitHubSettings } from "@/components/settings/GitHubSettings"
import { SettingsSection } from "@/components/settings/SettingRow"
import { YouTrackSettings } from "@/components/settings/YouTrackSettings"

/**
 * Settings → Accounts: the sign-ins of the app, GitHub's token and YouTrack's, read on each open of the window and on a
 * sign-in or sign-out of any window (`useAuthStatusOnOpen` of the Settings page, `useAuthSync` of every window). The
 * sign-ins of the command-line tools are their own, on Tools
 */
export function AccountsPage() {
  return (
    <>
      <SettingsSection title="GitHub" description="Pull requests and the requests for your review" focusKey="account-github">
        <GitHubSettings />
      </SettingsSection>
      <SettingsSection title="YouTrack" description="Hover an issue key to see the issue, and mark it fixed once its work lands" focusKey="account-youtrack">
        <YouTrackSettings />
      </SettingsSection>
    </>
  )
}
