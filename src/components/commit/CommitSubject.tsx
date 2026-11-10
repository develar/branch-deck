import type { HTMLAttributes } from "react"
import { cn } from "cn"
import { LinkedText } from "@/components/commit/LinkedText"
import { splitSubjectPrefix } from "@/lib/commit"
import type { SplitSubject } from "@/lib/commit"

/** The prefix of a subject as every place sets it: in the code font at the size of the text, dimmed, of regular weight */
const PREFIX_CLASS = "font-mono font-normal text-muted-foreground"

/**
 * A title with its conventional-commit prefix apart (`ReviewTitle` of Rust, or `splitSubjectPrefix`): the prefix dimmed
 * (`prefixClassName`), then the text, with issue links unless `plain`: a title inside a button holds no link
 * (docs/contributing/ux-writing.md), the button that opens its object is beside it
 */
export function TitleText({ title, prefixClassName = PREFIX_CLASS, plain = false }: { title: SplitSubject; prefixClassName?: string; plain?: boolean }) {
  return (
    <>
      {title.prefix != null && (
        <>
          <span data-slot="commit-subject-prefix" className={prefixClassName}>
            {title.prefix}
          </span>{" "}
        </>
      )}
      {plain ? title.text : <LinkedText text={title.text} />}
    </>
  )
}

/** The subject with issue links and its conventional-commit prefix dimmed (`prefixClassName`) */
export function CommitSubjectText({ subject, prefixClassName }: { subject: string; prefixClassName: string }) {
  return <TitleText title={splitSubjectPrefix(subject)} prefixClassName={prefixClassName} />
}

/**
 * Commit subject with issue links; a conventional-commit prefix (`docs air:`, `feat(model):`) is kept but dimmed, in
 * monospace at the size of the subject (JetBrains Mono and Inter share the x-height, as `InlineCode`).
 * Wraps to two lines at most unless `clamp` is false (the commit preview shows it whole).
 * Regular weight: in a list the subject is content, and weight marks names and titles. A preview, where the subject is
 * the title, passes `font-medium` in `className`; the prefix stays regular.
 */
export function CommitSubject({ subject, clamp = true, className, ...props }: { subject: string; clamp?: boolean; className?: string } & HTMLAttributes<HTMLSpanElement>) {
  return (
    <span {...props} className={cn(clamp && "line-clamp-2", "text-sm text-pretty text-foreground", className)}>
      <CommitSubjectText subject={subject} prefixClassName={PREFIX_CLASS} />
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
      <CommitSubjectText subject={subject} prefixClassName="font-mono text-muted-foreground" />
    </span>
  )
}
