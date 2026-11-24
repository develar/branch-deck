import { useRef, useState } from "react"
import { Kbd } from "@/components/ui/kbd"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import type { UnattendedDenial } from "@/lib/bindings"
import { formatTimestamp } from "@/lib/time"
import { deniedAgent, deniedCount, deniedPlace } from "@/presenters/unattended"
import { closeDeniedRequests, useDeniedRequests } from "@/stores/unattended"

/**
 * Denied Requests (openspec/specs/repository/unattended-mode/spec.md): the permission requests the hook of Unattended Mode
 * denied while the mode was on, oldest first, each with its time, the tool, what it asked for, who asked and where.
 * Opened by the toast of a switch off; kept mounted while closed, as the frame needs
 */
export function DeniedRequestsDialog() {
  const denials = useDeniedRequests()
  // the list stays while the closing animation runs
  const [shown, setShown] = useState<readonly UnattendedDenial[]>([])
  if (denials !== null && denials !== shown) {
    setShown(denials)
  }
  const bodyRef = useRef<HTMLDivElement>(null)
  return (
    <PreviewDialogFrame
      data-testid="denied-requests-dialog"
      width="lg"
      open={denials !== null}
      onOpenChange={nextOpen => {
        if (!nextOpen) {
          closeDeniedRequests()
        }
      }}
      initialFocus={bodyRef}
    >
      <PreviewDialogHeader>
        <h2 className="text-base font-semibold">Denied Requests</h2>
        <p className="text-sm text-muted-foreground">
          {`${deniedCount(shown.length)} while Unattended Mode was on. Allow in the settings of Claude Code what the agents need next time.`}
        </p>
      </PreviewDialogHeader>
      <PreviewDialogBody ref={bodyRef} data-testid="denied-requests-body">
        <ol className="flex flex-col gap-3">
          {shown.map((denial, index) => (
            <li key={`${denial.time}-${index}`} className="flex min-w-0 flex-col gap-0.5" data-testid="denied-request">
              <span className="flex min-w-0 items-baseline gap-2">
                <span className="shrink-0 font-medium">{denial.tool_name}</span>
                <span className="min-w-0 truncate font-mono text-xs" title={denial.summary}>
                  {denial.summary}
                </span>
              </span>
              <span className="min-w-0 truncate text-xs text-muted-foreground" title={denial.cwd}>
                {`${formatTimestamp(denial.time)} · ${deniedAgent(denial)} · ${deniedPlace(denial)}`}
              </span>
            </li>
          ))}
        </ol>
      </PreviewDialogBody>
      <PreviewDialogFooter
        start={
          <span>
            <code className="font-mono">brd unattended status</code> lists them too
          </span>
        }
      >
        <KeyHint keys={<Kbd>Esc</Kbd>}>Close</KeyHint>
      </PreviewDialogFooter>
    </PreviewDialogFrame>
  )
}
