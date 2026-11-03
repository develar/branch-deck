import type { ComponentProps } from "react"
import { cn } from "cn"

export interface ReadingColumnProps extends ComponentProps<"div"> {
  /** `section` for a document with a name of its own (the overview) */
  as?: "div" | "section"
}

/**
 * Prose a person reads in a wide pane (docs/design/ui-theme.md, "Reading column"): the document of a review, the
 * opening of a step, the overview of a pull request. One measure for every block in it (`max-w-measure`, about 80
 * characters), so headings, paragraphs and their facts end at one edge; the column in the middle of its pane, the
 * reading size (`text-md`), the lines wrapped to avoid a short last one (`text-pretty`). A block inside sets no width
 * of its own
 */
export function ReadingColumn({ as: Element = "div", className, ...props }: ReadingColumnProps) {
  return <Element className={cn("mx-auto flex w-full max-w-measure min-w-0 flex-col text-md text-pretty", className)} {...props} />
}
