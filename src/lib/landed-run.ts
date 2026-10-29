// The dismissal of the report of what landed (spec/repository/landed-commits.spec.md): the intent `dismissLanded` of the
// session, which writes the report under its lock for every window and `bd`; this window hides it at once.
import { Channel } from "@tauri-apps/api/core"
import type { OperationEvent } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { executeIntent } from "@/model/intents"
import { clearNewlyArchived } from "@/stores/archived-branches"
import { clearLandedReport } from "@/stores/landed"

/** The user has seen the report: cleared for the app, other windows and `bd`, with the "New" marks of the archive */
export async function dismissLanded(repositoryPath: string): Promise<void> {
  clearNewlyArchived()
  clearLandedReport(repositoryPath)
  // an operation of the session (`Intent::DismissLanded`), which writes the report under its lock for every window and `bd`
  const intent = { kind: "dismissLanded" } as const
  const result = await invokeCommand(() => executeIntent({ repositoryPath, intent, fingerprint: "", accepted: [] }, new Channel<OperationEvent>()))
  if (result.status !== "ok") {
    notifyError("Cannot dismiss the landed commits", result.error)
  }
}
