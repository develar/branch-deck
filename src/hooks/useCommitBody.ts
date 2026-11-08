import { useEffect, useState } from "react"
import type { CommitBody } from "@/lib/bindings"
import { commands } from "@/lib/bindings"

/** The messages whose bodies are kept: the commits a review or a preview shows at a time, and some back */
const KEPT_MESSAGES = 200

/** The body of an empty message, and of one Rust failed to read */
const NO_BODY: CommitBody = { blocks: [], trailers: [], lead: false, summary: null, parts: [], code: [] }

/** The bodies Rust read of each message: a message is its own key, the grammar of a commit never changes */
const bodyOf = new Map<string, CommitBody>()

function keep(message: string, body: CommitBody): void {
  bodyOf.set(message, body)
  if (bodyOf.size > KEPT_MESSAGES) {
    bodyOf.delete(bodyOf.keys().next().value!)
  }
}

/**
 * The body of `message` (the lines after its subject line) as Rust reads it (`commit_body`, the grammar the body of a
 * pull request is written from): its blocks with their labels and headings, its trailers, its lead and its fold. Asked
 * once per message, kept for the ones shown again; undefined until Rust answered, an empty body on a failed read. A
 * null message (a body Rust read already) asks nothing
 */
export function useCommitBody(message: string | null): CommitBody | undefined {
  const [answered, setAnswered] = useState<{ of: string; body: CommitBody } | null>(null)
  const kept = message === null ? undefined : bodyOf.get(message)
  useEffect(() => {
    if (message === null || bodyOf.has(message)) {
      return
    }
    let current = true
    commands.commitBody({ message }).then(
      body => {
        keep(message, body)
        if (current) {
          setAnswered({ of: message, body })
        }
      },
      () => {
        if (current) {
          setAnswered({ of: message, body: NO_BODY })
        }
      },
    )
    return () => {
      current = false
    }
  }, [message])
  return kept ?? (answered !== null && answered.of === message ? answered.body : undefined)
}
