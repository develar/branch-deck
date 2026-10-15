/**
 * Shared UI color types for consistent theming across all components.
 * Mirrors the semantic tones used by the badge and alert variants.
 */
export type UIColor = "primary" | "secondary" | "success" | "info" | "warning" | "error" | "neutral"

export type SubtleBadgeVariant = "primary" | "secondary" | "success" | "info" | "warning" | "destructive"

/** Badge variant with a tinted background for a semantic color. */
export function subtleBadgeVariant(color: UIColor): SubtleBadgeVariant {
  switch (color) {
    case "primary":
      return "primary"
    case "success":
      return "success"
    case "info":
      return "info"
    case "warning":
      return "warning"
    case "error":
      return "destructive"
    case "secondary":
    case "neutral":
      return "secondary"
  }
}

/**
 * Text colour of a ghost button, for an action that repeats on every row and must stay quiet. The colour stays while
 * the button's popover is open (`aria-expanded`), which the ghost variant would turn to the foreground. A routine
 * action (`primary`) colours only its icon, so a fully coloured label in the column means "look here".
 */
export function ghostButtonColorClass(color: UIColor): string {
  switch (color) {
    case "primary":
      return "text-foreground hover:bg-primary/10 aria-expanded:bg-primary/10 aria-expanded:text-foreground [&_svg]:text-primary"
    case "success":
      return "text-success hover:bg-success/10 hover:text-success aria-expanded:bg-success/10 aria-expanded:text-success"
    case "info":
      return "text-info hover:bg-info/10 hover:text-info aria-expanded:bg-info/10 aria-expanded:text-info"
    case "warning":
      return "text-warning hover:bg-warning/10 hover:text-warning aria-expanded:bg-warning/10 aria-expanded:text-warning"
    case "error":
      return "text-destructive hover:bg-destructive/10 hover:text-destructive aria-expanded:bg-destructive/10 aria-expanded:text-destructive"
    case "secondary":
    case "neutral":
      return "text-muted-foreground"
  }
}

/** Colour of the dot before a routine status text */
export function statusDotClassName(color: UIColor): string {
  switch (color) {
    case "primary":
      return "bg-primary"
    case "success":
      return "bg-success"
    case "info":
      return "bg-info"
    case "warning":
      return "bg-warning"
    case "error":
      return "bg-destructive"
    default:
      return "bg-muted-foreground/50"
  }
}
