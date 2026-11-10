import { TitleText } from "@/components/commit/CommitSubject"
import { BreakableCode } from "@/components/shared/CodeText"
import type { ReviewTitle } from "@/lib/bindings"

/**
 * The title of a step or of a commit of the review (`Chapter.title`), one way in the tree, the contents, the opening and
 * the bar of the step (openspec/specs/branch/review-tour/spec.md, "Rust titles each commit without the issue key of the
 * brief"): the prefix Rust split off in the code font and dimmed, then the text; a code name (`code`) wraps at its dots
 * and humps. No link: the title is the text of a button that shows its step
 */
export function ReviewTitleText({ title, code = false }: { title: ReviewTitle; code?: boolean }) {
  return code ? <BreakableCode text={title.text} humps /> : <TitleText title={title} plain />
}
