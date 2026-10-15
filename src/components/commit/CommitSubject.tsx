import { cn } from "cn"
import { LinkedText } from "@/components/commit/LinkedText"
import { splitSubjectPrefix } from "@/lib/commit"

/** The subject with issue links and its conventional-commit prefix dimmed (`prefixClassName`) */
function SubjectText({ subject, prefixClassName }: { subject: string, prefixClassName: string }) {
  const { prefix, text } = splitSubjectPrefix(subject)
  return (
    <>
      {prefix != null && (
        <>
          <span data-slot="commit-subject-prefix" className={prefixClassName}>{prefix}</span>
          {" "}
        </>
      )}
      <LinkedText text={text} />
    </>
  )
}

/**
 * Commit subject with issue links; a conventional-commit prefix (`docs air:`, `feat(model):`) is kept but dimmed.
 * Wraps to two lines at most unless `clamp` is false (the commit preview shows it whole).
 * Regular weight: in a list the subject is content, and weight marks names and titles. A preview, where the subject is
 * the title, passes `font-medium` in `className`; the prefix stays regular.
 */
export function CommitSubject({ subject, clamp = true, className }: { subject: string, clamp?: boolean, className?: string }) {
  return (
    <span className={cn(clamp && "line-clamp-2", "text-sm text-pretty text-foreground", className)}>
      <SubjectText subject={subject} prefixClassName="font-mono text-xs font-normal text-muted-foreground" />
    </span>
  )
}

/**
 * The subject in one truncated line of a compact list (a popover, the landed or dropped commits), in the size around
 * it: issue links and the dimmed prefix, as `CommitSubject` has them.
 */
export function CommitSubjectLine({ subject }: { subject: string }) {
  return (
    <span className="truncate">
      <SubjectText subject={subject} prefixClassName="font-mono text-muted-foreground" />
    </span>
  )
}
