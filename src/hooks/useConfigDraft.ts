import { useEffect, useEffectEvent, useState } from "react"
import type { KeyboardEvent } from "react"
import { FieldWrites } from "@/lib/field-writes"
import type { WriteAnswer, WriteFailure } from "@/lib/field-writes"
import { onSettingFieldKey } from "@/lib/setting-field"
import { saveBeforeClose } from "@/lib/settings-close"
import type { SettingNotSaved } from "@/lib/settings-close"
import { getSelectedProject } from "@/stores/repository"

export interface ConfigDraft {
  readonly value: string
  readonly onChange: (value: string) => void
  readonly onBlur: () => void
  readonly onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void
  /** What is wrong with the typing, said under the field: nothing was written yet, so no "Cannot …" */
  readonly error: string | null
}

/**
 * The typing of a text field of the git config (the branch prefix and the GitHub repository of a repository, the push
 * size limit): saved on Enter, when the field is left and when the window closes (`saveBeforeClose`, at once beside a
 * write that runs), never on a key; Escape takes it back. `shown` is the value the field shows without typing;
 * `normalize` makes the typing what the backend takes, `invalidReason` refuses it in place as the backend would; `write`
 * gets the value, null for empty; `notSaved` is what the main window says when the close cannot finish the save. The
 * typing is written only for the repository it began in (`path`, empty for the global config alone): a switch of the
 * repository writes nothing
 */
export function useConfigDraft({
  path,
  shown,
  normalize,
  invalidReason,
  write,
  notSaved,
}: {
  path: string
  shown: string
  normalize: (typing: string) => string
  invalidReason: (value: string) => string | null
  write: (value: string | null) => Promise<WriteAnswer>
  notSaved: SettingNotSaved
}): ConfigDraft {
  const [draft, setDraft] = useState<string | null>(null)
  // one write of one change, whichever of a blur, Enter and the close of the window saves it (`FieldWrites`)
  const [writes] = useState(() => new FieldWrites())
  const typed = draft === null ? null : normalize(draft)
  const error = typed === null || typed === "" ? null : invalidReason(typed)
  useEffect(() => {
    if (draft === null) {
      writes.typingCleared()
    }
  }, [draft, writes])

  const save = (now: boolean): Promise<WriteFailure | null> => {
    // another repository was selected meanwhile: its page replaces this one, nothing is written
    if (typed === null || error !== null || (getSelectedProject()?.path ?? "") !== path) {
      return Promise.resolve(null)
    }
    // whether there is anything to write is the writes' to say: while one runs, the field shows the value before it
    return writes.save(
      typed,
      shown,
      () => write(typed === "" ? null : typed),
      // an edit typed while the write ran stays, and is written after it
      value => setDraft(current => (current !== null && normalize(current) === value ? null : current)),
      now,
    )
  }

  // a window closed while the field holds valid typing saves it first, at once
  const saveOnClose = useEffectEvent(() => save(true))
  const notSavedOnClose = useEffectEvent(() => notSaved)
  useEffect(() => saveBeforeClose(saveOnClose, notSavedOnClose), [])

  return {
    value: draft ?? shown,
    onChange: setDraft,
    onBlur: () => void save(false),
    onKeyDown: event => onSettingFieldKey(event, () => void save(false), draft === null ? null : () => setDraft(null)),
    error,
  }
}
