import { useEffect, useState } from "react"
import type { CommitBodyBlock } from "@/lib/bindings"
import { commands } from "@/lib/bindings"

/** The messages whose blocks are kept: the commits a review or a preview shows at a time, and some back */
const KEPT_MESSAGES = 200

/** The blocks Rust parsed of each message: a message is its own key, the parse of a commit never changes */
const blocksOf = new Map<string, readonly CommitBodyBlock[]>()

function keep(message: string, blocks: readonly CommitBodyBlock[]): void {
  blocksOf.set(message, blocks)
  if (blocksOf.size > KEPT_MESSAGES) {
    blocksOf.delete(blocksOf.keys().next().value!)
  }
}

/**
 * The blocks of the body of `message` (the lines after its subject line) as Rust parses it (`commit_body_blocks`, the
 * grammar the body of a pull request is written from): asked once per message, kept for the ones shown again;
 * undefined until Rust answered, an empty body on a failed read
 */
export function useCommitBodyBlocks(message: string): readonly CommitBodyBlock[] | undefined {
  const [answered, setAnswered] = useState<{ of: string; blocks: readonly CommitBodyBlock[] } | null>(null)
  const kept = blocksOf.get(message)
  useEffect(() => {
    if (blocksOf.has(message)) {
      return
    }
    let current = true
    commands.commitBodyBlocks({ message }).then(
      blocks => {
        keep(message, blocks)
        if (current) {
          setAnswered({ of: message, blocks })
        }
      },
      () => {
        if (current) {
          setAnswered({ of: message, blocks: [] })
        }
      },
    )
    return () => {
      current = false
    }
  }, [message])
  return kept ?? (answered?.of === message ? answered.blocks : undefined)
}

/** Forgets the kept blocks (tests) */
export function resetCommitBodyBlocks(): void {
  blocksOf.clear()
}
