import type { ComponentProps } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"

/** Soft primary: a tinted primary background and primary text, for the chosen chip and a suggested action ("Enable AI") */
export const softPrimaryClassName = "bg-primary/10 font-medium text-primary hover:bg-primary/20 hover:text-primary"

export interface ChoiceChipProps extends Omit<ComponentProps<typeof Button>, "variant" | "size"> {
  /** The input holds this chip's name */
  chosen: boolean
}

/**
 * An `xs` chip that puts its name into the input: an existing branch or an AI suggestion below the "Move to…" input.
 * The chosen chip is soft primary, the others `secondary`.
 */
export function ChoiceChip({ chosen, className, ...props }: ChoiceChipProps) {
  return (
    <Button
      size="xs"
      variant={chosen ? "ghost" : "secondary"}
      aria-pressed={chosen}
      className={cn(chosen && softPrimaryClassName, className)}
      {...props}
    />
  )
}
