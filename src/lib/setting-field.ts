import type { KeyboardEvent } from "react"

/**
 * The keys of a field of Settings that saves on Enter and when it is left, never on a key: Enter saves, and Escape
 * takes the typing back while there is some (`revert`, null without typing); the window stays open then, else Escape
 * closes it
 */
export function onSettingFieldKey(event: KeyboardEvent<HTMLInputElement>, save: () => void, revert: (() => void) | null): void {
  if (event.key === "Enter") {
    event.preventDefault()
    save()
  } else if (event.key === "Escape" && revert !== null) {
    event.preventDefault()
    event.stopPropagation()
    revert()
  }
}
