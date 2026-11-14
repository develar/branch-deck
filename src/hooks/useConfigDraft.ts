import { useEffect, useEffectEvent, useState } from "react"
import type { KeyboardEvent } from "react"
import { FieldWrites } from "@/lib/field-writes"
import { onSettingFieldKey } from "@/lib/setting-field"
import { saveBeforeClose } from "@/lib/settings-close"
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
 * size limit): saved on Enter, when the field is left and when the window closes (`saveBeforeClose`), never on a key;
 * Escape takes it back. `shown` is the value the field shows without typing; `normalize` makes the typing what the
 * backend takes, `invalidReason` refuses it in place as the backend would; `write` gets the value, null for empty. The
 * typing is written only for the repository it began in (`path`, empty for the global config alone): a switch of the
 * repository writes nothing
 */
export function useConfigDraft({
  path,
  shown,
  normalize,
  invalidReason,
  write,
}: {
  path: string
  shown: string
  normalize: (typing: string) => string
  invalidReason: (value: string) => string | null
  write: (value: string | null) => Promise<boolean>
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

  const save = (): Promise<void> => {
    // another repository was selected meanwhile: its page replaces this one, nothing is written
    if (typed === null || error !== null || (getSelectedProject()?.path ?? "") !== path) {
      return Promise.resolve()
    }
    if (typed === shown) {
      setDraft(null)
      return Promise.resolve()
    }
    return writes.save(
      typed,
      () => write(typed === "" ? null : typed),
      // an edit typed while the write ran stays, and is written after it
      value => setDraft(current => (current !== null && normalize(current) === value ? null : current)),
    )
  }

  // a window closed while the field holds valid typing saves it first
  const saveOnClose = useEffectEvent(() => save())
  useEffect(() => saveBeforeClose(() => saveOnClose()), [])

  return {
    value: draft ?? shown,
    onChange: setDraft,
    onBlur: () => void save(),
    onKeyDown: event => onSettingFieldKey(event, () => void save(), draft === null ? null : () => setDraft(null)),
    error,
  }
}
