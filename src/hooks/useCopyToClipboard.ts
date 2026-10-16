import { useCallback, useEffect, useRef, useState } from "react"
import { copyText } from "@/lib/clipboard"

/** How long the "copied" state lasts; E2E tests run the fake clock past it (`page.clock.fastForward`) */
const COPIED_DURATION_MS = 2000

/** Copies text to the clipboard and exposes a transient "copied" state (also used to open a confirmation tooltip). */
export function useCopyToClipboard() {
  const [isCopied, setIsCopied] = useState(false)
  const [tooltipOpen, setTooltipOpen] = useState(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(
    () => () => {
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current)
      }
    },
    [],
  )

  const copyToClipboard = useCallback(async (text: string) => {
    if (!(await copyText(text))) {
      return
    }
    setIsCopied(true)
    setTooltipOpen(true)

    if (timerRef.current !== null) {
      clearTimeout(timerRef.current)
    }
    timerRef.current = setTimeout(() => {
      timerRef.current = null
      setIsCopied(false)
      setTooltipOpen(false)
    }, COPIED_DURATION_MS)
  }, [])

  return {
    isCopied,
    tooltipOpen,
    setTooltipOpen,
    copyToClipboard,
  }
}
