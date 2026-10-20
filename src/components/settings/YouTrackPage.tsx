import { useEffect } from "react"
import { SettingsSection } from "@/components/settings/SettingRow"
import { YouTrackSettings } from "@/components/settings/YouTrackSettings"
import { useTauriListen } from "@/hooks/useTauriListen"
import { refreshYouTrackAuthStatus } from "@/stores/youtrack"

/**
 * Settings → YouTrack: the sign-in, asked again on each open of the window (`openCount`) and on a sign-in or sign-out of
 * any window (`youtrack-auth-changed`)
 */
export function YouTrackPage({ openCount }: { openCount: number }) {
  useEffect(() => {
    void refreshYouTrackAuthStatus()
  }, [openCount])
  useTauriListen("youtrack-auth-changed", () => {
    void refreshYouTrackAuthStatus()
  })
  return (
    <SettingsSection title="YouTrack" description="The summary and the state of an issue on its key, and Mark Fixed">
      <YouTrackSettings />
    </SettingsSection>
  )
}
