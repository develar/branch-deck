import { useState } from "react"

/** Which folded parts of a body a reader opened, and the part a click elsewhere opened (`revealed`, by the serial of that click) */
export interface BodyFold {
  readonly opened: ReadonlySet<string>
  /** A part opened or closed by hand, which plays no click elsewhere */
  readonly toggle: (key: string) => void
  readonly revealed: { readonly key: string; readonly serial: number } | null
}

/** The fold of a body and how a click elsewhere opens one of its parts (`reveal`, a mark of a section of the diffs) */
export interface BodyFoldState extends BodyFold {
  /** Opens the part of `key` for the click of `serial`, unless the reader opened it already; called while rendering */
  readonly reveal: (key: string, serial: number) => void
}

/** The folded parts of a body a reader opened, each by its key (`BodyFold`, `BodyPartsView`) */
export function useBodyFold(): BodyFoldState {
  const [opened, setOpened] = useState<ReadonlySet<string>>(() => new Set())
  const [revealed, setRevealed] = useState<BodyFold["revealed"]>(null)
  const toggle = (key: string) => {
    setRevealed(null)
    setOpened(previous => {
      const next = new Set(previous)
      if (!next.delete(key)) {
        next.add(key)
      }
      return next
    })
  }
  const reveal = (key: string, serial: number) => {
    if (!opened.has(key)) {
      setOpened(new Set([...opened, key]))
      setRevealed({ key, serial })
    }
  }
  return { opened, toggle, revealed, reveal }
}
