import { useEffect, useEffectEvent, useRef, useState } from "react"
import type { KeyboardEvent } from "react"
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
  const saving = useRef(false)
  const typed = draft === null ? null : normalize(draft)
  const error = typed === null || typed === "" ? null : invalidReason(typed)

  const save = (): Promise<void> => {
    // another repository was selected meanwhile: its page replaces this one, nothing is written; a blur after Enter while
    // its write runs writes nothing again
    if (typed === null || error !== null || saving.current || (getSelectedProject()?.path ?? "") !== path) {
      return Promise.resolve()
    }
    if (typed === shown) {
      setDraft(null)
      return Promise.resolve()
    }
    saving.current = true
    return write(typed === "" ? null : typed).then(written => {
      saving.current = false
      if (written) {
        setDraft(null)
      }
    })
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
