import { cn } from "cn"
import { LinkedText } from "@/components/commit/LinkedText"
import { splitSubjectPrefix } from "@/lib/commit"

/**
 * Commit subject with issue links; a conventional-commit prefix (`docs air:`, `feat(model):`) is kept but dimmed.
 * Wraps to two lines at most unless `clamp` is false (the commit preview shows it whole).
 */
export function CommitSubject({ subject, clamp = true }: { subject: string, clamp?: boolean }) {
  const { prefix, text } = splitSubjectPrefix(subject)
  return (
    <span className={cn(clamp && "line-clamp-2", "text-sm font-medium text-pretty text-foreground")}>
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
