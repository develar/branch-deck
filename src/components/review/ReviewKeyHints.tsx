import { useEffect, useState } from "react"
import { Kbd } from "@/components/ui/kbd"
import { KeyHint } from "@/components/shared/PreviewDialogFrame"
import { isMac } from "@/lib/platform"
import { REVIEW_KEYS } from "@/lib/review-keys"

export interface ReviewKeyHintsProps {
  /** ⌥↑ ⌥↓ step through more than one commit */
  commits: boolean
  /** What Escape does: `Close` in a window of its own, `Overview` in a pull request */
  escape: string
}

/** A row of a tree (`FileTree`): its keys are its own */
function isTreeRow(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && target.dataset.treeKey !== undefined
}

/** Whether a row of the tree of the files has the focus */
function useTreeRowFocused(): boolean {
  const [focused, setFocused] = useState(() => isTreeRow(document.activeElement))
  useEffect(() => {
    const focusIn = (event: FocusEvent) => setFocused(isTreeRow(event.target))
    // the focus goes nowhere (a click on no control): no row has it
    const focusOut = (event: FocusEvent) => setFocused(isTreeRow(event.relatedTarget))
    document.addEventListener("focusin", focusIn)
    document.addEventListener("focusout", focusOut)
    return () => {
      document.removeEventListener("focusin", focusIn)
      document.removeEventListener("focusout", focusOut)
    }
  }, [])
  return focused
}

/**
 * The keys of a review, the same on every surface: the files, the changes, the commits and Escape; while a row of the
 * tree has the focus, `I` for the card of its mark
 */
export function ReviewKeyHints({ commits, escape }: ReviewKeyHintsProps) {
  const option = isMac ? "⌥" : "Alt+"
  const treeRowFocused = useTreeRowFocused()
  return (
    <>
      <KeyHint
        keys={
          <>
            <Kbd>{REVIEW_KEYS.nextSection}</Kbd>
            <Kbd>{REVIEW_KEYS.previousSection}</Kbd>
          </>
        }
      >
        Files
      </KeyHint>
      <KeyHint
        keys={
          <>
            <Kbd>{REVIEW_KEYS.nextChange}</Kbd>
            <Kbd>{REVIEW_KEYS.previousChange}</Kbd>
          </>
        }
      >
        Changes
      </KeyHint>
      <KeyHint keys={<Kbd>{REVIEW_KEYS.sidebar}</Kbd>}>Sidebar</KeyHint>
      {treeRowFocused && <KeyHint keys={<Kbd>I</Kbd>}>Details</KeyHint>}
      {commits && (
        <KeyHint
          keys={
            <>
              <Kbd>{`${option}↑`}</Kbd>
              <Kbd>{`${option}↓`}</Kbd>
            </>
          }
        >
          Commits
        </KeyHint>
      )}
      <KeyHint keys={<Kbd>Esc</Kbd>}>{escape}</KeyHint>
    </>
  )
}
