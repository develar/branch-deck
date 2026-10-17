import { useEffect } from "react"
import { GitHubSettings } from "@/components/settings/GitHubSettings"
import { SettingsSection } from "@/components/settings/SettingRow"
import { useTauriListen } from "@/hooks/useTauriListen"
import { refreshGitHubAuthStatus } from "@/stores/github-auth"

/**
 * Settings → GitHub: where the token comes from and the app's own sign-in, asked again on each open of the window
 * (`openCount`) and on a sign-in or sign-out of any window (`github-auth-changed`)
 */
export function GitHubPage({ openCount }: { openCount: number }) {
  useEffect(() => {
    void refreshGitHubAuthStatus()
  }, [openCount])
  useTauriListen("github-auth-changed", () => {
    void refreshGitHubAuthStatus()
  })
  return (
    <SettingsSection title="GitHub" description="Pull requests and the requests for your review">
      <GitHubSettings />
    </SettingsSection>
  )
}
