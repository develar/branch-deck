import { openUrl } from "@tauri-apps/plugin-opener"
import { notifyError } from "@/lib/errors"

/**
 * Opens a Space merge request in the browser: one Patronus found open still counts for a Safe Push, although Branch
 * Deck creates none (deprecated: the approvals go into the GitHub pull request)
 */
export function openMergeRequest(url: string): void {
  openUrl(url).catch((error: unknown) => notifyError("Cannot open the merge request", error))
}
