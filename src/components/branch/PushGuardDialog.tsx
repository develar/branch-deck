import { useState } from "react"
import { RotateCcwIcon, ShieldOffIcon, TriangleAlertIcon, UploadIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
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
  const mode = shown === null ? "Safe Push" : formatPushMode(shown.robot.pushMode)
  const startsRobot = shown?.startsRobot === true
  return (
    <Dialog open={request !== null} onOpenChange={open => !open && answerPushGuard("back")}>
      <DialogContent className="sm:max-w-lg" data-testid="push-guard-dialog">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <TriangleAlertIcon className="size-4 text-warning" />
            {`${mode} of ${shown?.branchName ?? ""} is running`}
          </DialogTitle>
          <DialogDescription>
            {`Patronus tests the commit it took at the start. A push into the branch makes the ${mode} fail when it ends ("branch was modified").`}
            {startsRobot && " The new Safe Push takes the pushed commit."}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => answerPushGuard("back")}>Back</Button>
          <Button variant="outline" onClick={() => answerPushGuard("push")}>
            <UploadIcon data-icon="inline-start" />
            Push Anyway
          </Button>
          {startsRobot
            ? (
                <Button autoFocus onClick={() => answerPushGuard("cancel-push")}>
                  <ShieldOffIcon data-icon="inline-start" />
                  Cancel and Push
                </Button>
              )
            : (
                <Button autoFocus onClick={() => answerPushGuard("cancel-push-restart")}>
                  <RotateCcwIcon data-icon="inline-start" />
                  Cancel, Push and Restart
                </Button>
              )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
