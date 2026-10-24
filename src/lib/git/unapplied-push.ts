import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { reportPushError } from "@/lib/git/push"
import { showToast } from "@/lib/toast-message"
import { branchSyncStore, syncBranches } from "@/stores/branch-sync"
import { branchSubject, pushLockWaitChannel, runOperation } from "@/stores/operations"
import { vcsRequestFactory } from "@/stores/repository"
import { getUnappliedBranch } from "@/stores/unapplied-branches"

/**
 * Pushes the unapplied branch `name` (its full name) to the remote under its own name, so the remote keeps a copy:
 * leased on the remote head of its status (none: the remote branch must not exist), then the sync that shows its new
 * remote status. It lands nothing; a pull request of its virtual branch stays as it was. A remote operation of the row
 */
export async function pushUnappliedBranch(name: string): Promise<void> {
  const branch = getUnappliedBranch(name)
  if (branch === undefined) {
    return
  }
  const failureTitle = `Cannot push ${branch.branch}`
  // the repository of the click: a push that waits for another one of the branch still goes there
  const request = vcsRequestFactory.createRequest()
  await runOperation(
    { id: "push", kind: "remote", subject: branchSubject(name), objects: [name], label: `Pushing ${branch.branch}`, repositoryPath: request.repositoryPath },
    async key => {
      const result = await invokeCommand(() =>
        commands.pushBranches(
          {
            repositoryPath: request.repositoryPath,
            myEmail: null,
            baselineBranch: branchSyncStore.getState().baselineBranch ?? "",
            branches: [{ branchName: branch.branch, expectedRemote: branch.remoteStatus?.remoteTip ?? null, unapplied: true }],
          },
          pushLockWaitChannel(key),
        ),
      )
      if (result.status === "error") {
        reportPushError(notifyError, branch.branch, result.error)
        return
      }
      if (result.data.type === "remoteMoved") {
        notifyError(failureTitle, `The remote branch ${name} changed since the last fetch: nothing was pushed. Its status shows it now; a push again replaces it`)
      } else {
        showToast({ title: `Pushed ${branch.branch}`, description: `The remote keeps it as ${name}` })
      }
      await syncBranches("afterRewrite")
    },
    reason => toast.add({ type: "info", title: failureTitle, description: reason }),
  )
}
