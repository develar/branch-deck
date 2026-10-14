import { useRef } from "react"
import { RotateCcwIcon, TriangleAlertIcon, UploadIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { formatPushMode } from "@/lib/patronus"
import { answerPushGuard, usePushGuardRequest } from "@/stores/push-guard"

/**
 * Asked before a push into a branch whose Safe Push runs: the push makes the robot fail at the end. Cancel it, push and
 * restart it on the pushed commit; push anyway; or go back (Escape).
 */
export function PushGuardDialog() {
  const request = usePushGuardRequest()
  const restartRef = useRef<HTMLButtonElement>(null)
  const mode = request === null ? "Safe Push" : formatPushMode(request.robot.pushMode)
  return (
    <PreviewDialogFrame
      data-testid="push-guard-dialog"
      width="md"
      open={request !== null}
      onOpenChange={open => !open && answerPushGuard("back")}
      initialFocus={restartRef}
    >
      <PreviewDialogHeader>
        <DialogTitle className="flex items-center gap-2 leading-5">
          <TriangleAlertIcon className="size-4 shrink-0 text-warning" />
          {`${mode} of ${request?.branchName ?? ""} is running`}
        </DialogTitle>
      </PreviewDialogHeader>

      <PreviewDialogBody>
        <DialogDescription>
          {`Patronus tests the commit it took at the start. A push into the branch makes the ${mode} fail when it ends ("branch was modified").`}
        </DialogDescription>
      </PreviewDialogBody>

      <PreviewDialogFooter>
        <KeyHint keys={<Kbd>Esc</Kbd>}>Back</KeyHint>
        <Button size="xs" variant="outline" onClick={() => answerPushGuard("push")}>
          <UploadIcon data-icon="inline-start" />
          Push Anyway
        </Button>
        <Button ref={restartRef} size="xs" onClick={() => answerPushGuard("cancel-push-restart")}>
          <RotateCcwIcon data-icon="inline-start" />
          Cancel, Push and Restart
        </Button>
      </PreviewDialogFooter>
    </PreviewDialogFrame>
  )
}
