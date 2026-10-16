import type { ReactNode } from "react"
import { Kbd } from "@/components/ui/kbd"
import { KeyHint } from "@/components/shared/PreviewDialogFrame"
import { isMac } from "@/lib/platform"

export interface ReviewKeyHintsProps {
  /** ⌥↑ ⌥↓ step through more than one commit */
  commits: boolean
  /** What Escape does: `Back` to the preview, `Close` in a window of its own, `Overview` in a pull request */
  escape?: string
  /** After Escape: the actions of the surface, as W Open in Window */
  children?: ReactNode
}

/** The keys of a review, the same on every surface: the files, the changes, the commits and Escape */
export function ReviewKeyHints({ commits, escape = "Back", children }: ReviewKeyHintsProps) {
  const option = isMac ? "⌥" : "Alt+"
  return (
    <>
      <KeyHint keys={(
        <>
          <Kbd>J</Kbd>
          <Kbd>K</Kbd>
        </>
      )}
      >
        Files
      </KeyHint>
      <KeyHint keys={(
        <>
          <Kbd>N</Kbd>
          <Kbd>P</Kbd>
        </>
      )}
      >
        Changes
      </KeyHint>
      {commits && (
        <KeyHint keys={(
          <>
            <Kbd>{`${option}↑`}</Kbd>
            <Kbd>{`${option}↓`}</Kbd>
          </>
        )}
        >
          Commits
        </KeyHint>
      )}
      <KeyHint keys={<Kbd>Esc</Kbd>}>{escape}</KeyHint>
      {children}
    </>
  )
}
