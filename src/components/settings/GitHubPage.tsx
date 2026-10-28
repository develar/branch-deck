import { GitHubSettings } from "@/components/settings/GitHubSettings"
import { SettingsSection } from "@/components/settings/SettingRow"
import { useAuthStatusRead } from "@/stores/auth"

/**
 * Settings → GitHub: where the token comes from and the app's own sign-in, read on each open of the window
 * (`openCount`) and on a sign-in or sign-out of any window (`auth-changed`)
 */
export function GitHubPage({ openCount }: { openCount: number }) {
  useAuthStatusRead(openCount)
  return (
    <SettingsSection title="GitHub" description="Pull requests and the requests for your review">
      <GitHubSettings />
    </SettingsSection>
  )
}
