import type { ComponentProps } from "react"
import { cn } from "cn"

export interface InlineCodeProps extends ComponentProps<"code"> {
  /** Only the monospace font, no chip: a code token that prose detected rather than the author marked */
  plain?: boolean
}

/**
 * Code inside a sentence: the monospace font at the size of the text around it, a `bg-muted` chip, breaks anywhere.
 * Every code chip in prose (help, commit bodies, hints) is an `InlineCode`; a command on its own line is a code block.
 */
export function InlineCode({ plain = false, className, ...props }: InlineCodeProps) {
  return <code data-slot="inline-code" className={cn("font-mono leading-none [overflow-wrap:anywhere]", !plain && "rounded-sm bg-muted px-1", className)} {...props} />
}
