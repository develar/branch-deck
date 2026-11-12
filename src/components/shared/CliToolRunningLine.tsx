import { openUrl } from "@tauri-apps/plugin-opener"
import { ExternalLinkIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { notifyError } from "@/lib/errors"
import type { RunningCliToolAction } from "@/stores/cli-tools"

/**
 * The last line of an action of a CLI that runs, in whichever window started it (openspec/specs/repository/cli-tools/spec.md):
 * its row in Settings → Tools, the item of the Setup dot; a sign-in also offers the page it opened, for a browser that
 * didn't come up
 */
export function CliToolRunningLine({ running }: { running: RunningCliToolAction }) {
  const url = running.url
  return (
    <p className="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground" aria-live="polite" data-testid="cli-tool-progress">
      <Spinner className="size-3 shrink-0" />
      <span className="min-w-0 truncate font-mono">{running.line === "" ? "Starting…" : running.line}</span>
      {url !== null && (
        <Button
          variant="link"
          size="xs"
          className="h-auto px-1 py-0"
          onClick={() => void openUrl(url).catch((error: unknown) => notifyError("Cannot open the TeamCity sign-in", error))}
        >
          <ExternalLinkIcon data-icon="inline-start" />
          Open in Browser
        </Button>
      )}
    </p>
  )
}
