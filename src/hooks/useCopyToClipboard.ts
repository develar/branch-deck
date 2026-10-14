import { useCallback, useEffect, useRef, useState } from "react"
import { copyText } from "@/lib/clipboard"

/**
 * Copies text to the clipboard and exposes a transient "copied" state (also used to open a confirmation tooltip).
 * The reset timeout is 2 s, overridable in E2E via `window.__BRANCH_DECK_TEST_CONFIG__.copyTimeout`.
 */
export function useCopyToClipboard() {
  const [isCopied, setIsCopied] = useState(false)
  const [tooltipOpen, setTooltipOpen] = useState(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => () => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current)
    }
  }, [])

  const copyToClipboard = useCallback(async (text: string) => {
    if (!await copyText(text)) {
      return
    }
    setIsCopied(true)
    setTooltipOpen(true)

    const timeout = window.__BRANCH_DECK_TEST_CONFIG__?.copyTimeout ?? 2000
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current)
    }
    timerRef.current = setTimeout(() => {
      timerRef.current = null
      setIsCopied(false)
      setTooltipOpen(false)
    }, timeout)
  }, [])

  return {
    isCopied,
    tooltipOpen,
    setTooltipOpen,
    copyToClipboard,
  }
}
