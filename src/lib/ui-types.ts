/**
 * Shared UI color types for consistent theming across all components.
 * Mirrors the semantic tones used by the badge and alert variants.
 */
export type UIColor = "primary" | "secondary" | "success" | "info" | "warning" | "error" | "neutral"

export type SubtleBadgeVariant = "primary" | "secondary" | "success" | "info" | "warning" | "destructive"

/** Badge variant with a tinted background for a semantic color (Nuxt UI `variant="soft"`/`"subtle"`). */
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

/** Text and border classes for an outline button in a semantic color (Nuxt UI `UButton variant="outline" :color`). */
export function outlineButtonColorClass(color: UIColor): string {
  switch (color) {
    case "primary":
      return "border-primary/50 text-primary hover:bg-primary/10 hover:text-primary"
    case "success":
      return "border-success/50 text-success hover:bg-success/10 hover:text-success"
    case "info":
      return "border-info/50 text-info hover:bg-info/10 hover:text-info"
    case "warning":
      return "border-warning/50 text-warning hover:bg-warning/10 hover:text-warning"
    case "error":
      return "border-destructive/50 text-destructive hover:bg-destructive/10 hover:text-destructive"
    case "secondary":
    case "neutral":
      return ""
  }
}
