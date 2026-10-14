import { openUrl } from "@tauri-apps/plugin-opener"
import { toast } from "@/components/ui/toast"
import { notifyError } from "@/lib/errors"
import { pushBranch } from "@/lib/git/push"
import { getBranch } from "@/stores/branch-sync"
import { createPatronusMergeRequest } from "@/stores/patronus"

/** Opens a Space merge request in the browser */
export function openMergeRequest(url: string): void {
  openUrl(url).catch((error: unknown) => notifyError("Cannot open the merge request", error))
}

/**
 * "Create Merge Request…" of the branch menu, the step of the Safe Push form on its own: pushes the branch when the
 * remote lacks its tip (Space opens a merge request of a pushed branch; the push asks first while a Safe Push of the
 * branch runs), creates the merge request into the target branch and opens it in the browser. A toast names it and
 * opens it again. A failed push creates nothing; a failure is an error toast.
 */
export async function createBranchMergeRequest(branchName: string): Promise<void> {
  const branch = getBranch(branchName)
  if (branch === undefined) {
    return
  }
  const pushed = (branch.remoteStatus?.remoteTip ?? null) === (branch.commits[0]?.hash ?? null)
  if (!pushed && !await pushBranch(branchName)) {
    return
  }

  const progressId = toast.add({ type: "loading", title: `Creating a merge request of ${branchName}…`, timeout: 0 })
  try {
    const mergeRequest = await createPatronusMergeRequest(branchName)
    toast.close(progressId)
    openMergeRequest(mergeRequest.url)
    toast.add({
      type: "success",
      title: `Created merge request ${mergeRequest.reviewKey}`,
      description: `${branchName}: the code owners approve it there, then a Safe Push counts their approvals.`,
      timeout: 10_000,
      actionProps: { children: "Open merge request", onClick: () => openMergeRequest(mergeRequest.url) },
    })
  }
  catch (error) {
    toast.close(progressId)
    notifyError("Cannot create the merge request", error)
  }
}
