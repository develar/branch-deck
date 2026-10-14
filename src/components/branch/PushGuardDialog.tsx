import { useRef, useState } from "react"
import { RotateCcwIcon, ShieldOffIcon, TriangleAlertIcon, UploadIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { formatPushMode } from "@/lib/patronus"
import { answerPushGuard, usePushGuardRequest } from "@/stores/push-guard"

/**
 * Asked before a push into a branch whose Safe Push runs: the push makes the robot fail at the end. Cancel it, push and
 * restart it on the pushed commit; push anyway; or go back (Escape). A push for a new Safe Push (the Safe Push form)
 * offers to cancel and push instead: the new robot takes the pushed commit, a restart would run the branch twice.
 */
export function PushGuardDialog() {
  const request = usePushGuardRequest()
  // the answered question stays in the dialog while it closes
  const [shown, setShown] = useState(request)
  if (request !== null && request !== shown) {
    setShown(request)
  }
  const primaryRef = useRef<HTMLButtonElement>(null)
  const mode = shown === null ? "Safe Push" : formatPushMode(shown.robot.pushMode)
  const startsRobot = shown?.startsRobot === true
  return (
    <PreviewDialogFrame
      data-testid="push-guard-dialog"
      width="md"
      open={request !== null}
      onOpenChange={open => !open && answerPushGuard("back")}
      initialFocus={primaryRef}
    >
      <PreviewDialogHeader>
        <DialogTitle className="flex items-center gap-2 leading-5">
          <TriangleAlertIcon className="size-4 shrink-0 text-warning" />
          {`${mode} of ${shown?.branchName ?? ""} is running`}
        </DialogTitle>
      </PreviewDialogHeader>

      <PreviewDialogBody>
        <DialogDescription>
          {`Patronus tests the commit it took at the start. A push into the branch makes the ${mode} fail when it ends ("branch was modified").`}
          {startsRobot && " The new Safe Push takes the pushed commit."}
        </DialogDescription>
      </PreviewDialogBody>

      <PreviewDialogFooter>
        <KeyHint keys={<Kbd>Esc</Kbd>}>Back</KeyHint>
        <Button size="xs" variant="outline" onClick={() => answerPushGuard("push")}>
          <UploadIcon data-icon="inline-start" />
          Push Anyway
        </Button>
        {startsRobot
          ? (
              <Button ref={primaryRef} size="xs" onClick={() => answerPushGuard("cancel-push")}>
                <ShieldOffIcon data-icon="inline-start" />
                Cancel and Push
              </Button>
            )
          : (
              <Button ref={primaryRef} size="xs" onClick={() => answerPushGuard("cancel-push-restart")}>
                <RotateCcwIcon data-icon="inline-start" />
                Cancel, Push and Restart
              </Button>
            )}
      </PreviewDialogFooter>
    </PreviewDialogFrame>
  )
}
