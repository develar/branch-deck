// TODO(Phase 8): placeholder; replaced by the full port of AIStatusIndicator.vue
//  (AI mode toggle on click, status/error/help popover, download progress popover with `ai-status-icon-downloading`).
import { SparklesIcon } from "lucide-react"

/** AI status icon next to the inline branch creator buttons. */
export function AIStatusIndicator() {
  return <SparklesIcon className="size-3.5 text-muted-foreground" data-testid="ai-status-icon" />
}
