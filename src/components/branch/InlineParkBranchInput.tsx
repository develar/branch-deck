import { useState } from "react"
import { PauseCircleIcon } from "lucide-react"
import { InlineInputDialog } from "@/components/shared/InlineInputDialog"
import { parkBranch } from "@/hooks/useParkBranch"
import { useParkedState } from "@/stores/branch-sync"
import { closeInline, inlineSurface, withRowProgress } from "@/stores/inline-row-action"

/** The longest note a park keeps (`MAX_NOTE_LENGTH` of `history_rewrite::parked`) */
const MAX_NOTE_LENGTH = 500

/**
 * Park Branch… and Edit Park Note…: the note says why the branch stays local ("waits for the fix of IJPL-1"), one line,
 * optional; Park writes it, the row moves into the Parked card
 */
export function InlineParkBranchInput({ branchName }: { branchName: string }) {
  const parked = useParkedState(branchName)
  // a parked branch edits its own note; one parked with its parent gets a line of its own
  const current = parked?.with === null ? parked.note : ""
  const editing = parked?.with === null
  const [note, setNote] = useState(current)
  const tooLong = note.trim().length > MAX_NOTE_LENGTH
  const unchanged = editing && note.trim() === current

  return (
    <InlineInputDialog
      value={note}
      onValueChange={setNote}
      placeholder="Why it stays local, e.g. waits for the fix of IJPL-123 (optional)"
      validationMessage={tooLong ? `At most ${MAX_NOTE_LENGTH} characters` : undefined}
      validationColor={tooLong ? "error" : "primary"}
      validationTextClassName={tooLong ? "text-destructive" : ""}
      canSubmit={!tooLong && !unchanged}
      title={editing ? `Edit the park note of ${branchName}` : `Park ${branchName}`}
      submitText={editing ? "Save" : "Park"}
      dataTestId="inline-park-input"
      leadingIcon={<PauseCircleIcon className="size-4 text-muted-foreground" />}
      helpText={
        editing
          ? "The note shows in the Parked card."
          : "The commits stay on your branch and in its build; no push, Safe Push or Dry Run sends the branch, and it is no move of yours until you unpark it."
      }
      onSubmit={() => {
        // a failure keeps the form with the typed note
        void withRowProgress(branchName, () => parkBranch(branchName, note.trim(), { surface: inlineSurface(branchName) }), { keepFormOnFailure: true })
      }}
      onCancel={closeInline}
    />
  )
}
