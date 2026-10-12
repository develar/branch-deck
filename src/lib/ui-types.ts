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

/** Text colour of a ghost button, for an action that repeats on every row and must stay quiet. */
export function ghostButtonColorClass(color: UIColor): string {
  switch (color) {
    case "primary":
      return "text-primary hover:bg-primary/10 hover:text-primary"
    case "success":
      return "text-success hover:bg-success/10 hover:text-success"
    case "info":
      return "text-info hover:bg-info/10 hover:text-info"
    case "warning":
      return "text-warning hover:bg-warning/10 hover:text-warning"
    case "error":
      return "text-destructive hover:bg-destructive/10 hover:text-destructive"
    case "secondary":
    case "neutral":
      return "text-muted-foreground"
  }
}
