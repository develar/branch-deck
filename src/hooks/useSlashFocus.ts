import { useEffect } from "react"
import type { RefObject } from "react"
import { KEY_OWNERS } from "@/lib/preview-keys"

/**
 * `/` focuses the search field from anywhere in the window and selects its text, while `enabled`; `/` keeps its meaning
 * in a text field, a menu and a dialog (`KEY_OWNERS`)
 */
export function useSlashFocus(inputRef: RefObject<HTMLInputElement | null>, enabled: boolean): void {
  useEffect(() => {
    if (!enabled) {
      return
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "/" || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) {
        return
      }
      if (event.target instanceof Element && event.target.closest(KEY_OWNERS) !== null) {
        return
      }
      event.preventDefault()
      inputRef.current?.focus()
      inputRef.current?.select()
    }
    document.addEventListener("keydown", onKeyDown)
    return () => document.removeEventListener("keydown", onKeyDown)
  }, [inputRef, enabled])
}
