import { RotateCcwIcon, TriangleAlertIcon, UploadIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { formatPushMode } from "@/lib/patronus"
import { answerPushGuard, usePushGuardRequest } from "@/stores/push-guard"

/**
 * Asked before a push into a branch whose Safe Push runs: the push makes the robot fail at the end. Cancel it, push and
 * restart it on the pushed commit; push anyway; or go back (Escape).
 */
export function PushGuardDialog() {
  const request = usePushGuardRequest()
  const mode = request === null ? "Safe Push" : formatPushMode(request.robot.pushMode)
  return (
    <Dialog open={request !== null} onOpenChange={open => !open && answerPushGuard("back")}>
      <DialogContent className="sm:max-w-lg" data-testid="push-guard-dialog">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <TriangleAlertIcon className="size-4 text-warning" />
            {`${mode} of ${request?.branchName ?? ""} is running`}
          </DialogTitle>
          <DialogDescription>
            {`Patronus tests the commit it took at the start. A push into the branch makes the ${mode} fail when it ends ("branch was modified").`}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => answerPushGuard("back")}>Back</Button>
          <Button variant="outline" onClick={() => answerPushGuard("push")}>
            <UploadIcon data-icon="inline-start" />
            Push Anyway
          </Button>
          <Button autoFocus onClick={() => answerPushGuard("cancel-push-restart")}>
            <RotateCcwIcon data-icon="inline-start" />
            Cancel, Push and Restart
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
