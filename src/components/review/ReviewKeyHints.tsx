import { Kbd } from "@/components/ui/kbd"
import { KeyHint } from "@/components/shared/PreviewDialogFrame"
import { isMac } from "@/lib/platform"

export interface ReviewKeyHintsProps {
  /** ⌥↑ ⌥↓ step through more than one commit */
  commits: boolean
  /** What Escape does: `Close` in a window of its own, `Overview` in a pull request */
  escape: string
}

/** The keys of a review, the same on every surface: the files, the changes, the commits and Escape */
export function ReviewKeyHints({ commits, escape }: ReviewKeyHintsProps) {
  const option = isMac ? "⌥" : "Alt+"
  return (
    <>
      <KeyHint
        keys={
          <>
            <Kbd>J</Kbd>
            <Kbd>K</Kbd>
          </>
        }
      >
        Files
      </KeyHint>
      <KeyHint
        keys={
          <>
            <Kbd>N</Kbd>
            <Kbd>P</Kbd>
          </>
        }
      >
        Changes
      </KeyHint>
      <KeyHint keys={<Kbd>F</Kbd>}>Sidebar</KeyHint>
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
