import type { ComponentProps } from "react"
import { cn } from "cn"

export interface InlineCodeProps extends ComponentProps<"code"> {
  /** Only the monospace font, no chip: a code token that prose detected rather than the author marked */
  plain?: boolean
}

/**
 * Code inside a sentence: the monospace font at the size of the text around it, breaks anywhere; a chip of `bg-muted`
 * whose halo (a ring of the same colour) takes no width, so the punctuation around it keeps its place ("(Frame.paint)",
 * never "( Frame.paint )"). Every code in prose (help, commit bodies, hints) is an `InlineCode`; a command on its own
 * line is a code block.
 */
export function InlineCode({ plain = false, className, ...props }: InlineCodeProps) {
  return <code data-slot="inline-code" className={cn("font-mono leading-none [overflow-wrap:anywhere]", !plain && "rounded-xs bg-muted ring-2 ring-muted", className)} {...props} />
}
