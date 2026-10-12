import { LinkedText } from "@/components/commit/LinkedText"
import { splitSubjectPrefix } from "@/lib/commit"

/** Commit subject with issue links; a conventional-commit prefix (`docs air:`, `feat(model):`) is kept but dimmed. */
export function CommitSubject({ subject }: { subject: string }) {
  const { prefix, text } = splitSubjectPrefix(subject)
  return (
    <span className="line-clamp-2 text-sm font-medium text-pretty text-foreground">
      {prefix != null && (
        <>
          <span data-slot="commit-subject-prefix" className="font-mono text-xs font-normal text-muted-foreground">{prefix}</span>
          {" "}
        </>
      )}
      <LinkedText text={text} />
    </span>
  )
}
