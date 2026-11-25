import { cn } from "cn"
import type { PriceRisk } from "@/presenters/landing-plan"

/**
 * The price of Land Together as one muted sentence of its risks, each with its whole in the hover: "Land Together: 1
 * robot instead of 2 · same 7 checks · a failure holds both" (`describeTogetherPrice`), wrapping between words as text
 * does; the frame of an offer in the plan and the Safe Push popover show the same
 */
export function PriceLine({ risks, lead, className, testId = "plan-price" }: { risks: readonly PriceRisk[]; lead?: string; className?: string; testId?: string }) {
  return (
    <p className={cn("text-xs text-muted-foreground", className)} data-testid={testId}>
      {lead !== undefined && `${lead} `}
      {risks.map((risk, index) => (
        <span key={risk.text}>
          {risk.title === undefined ? (
            risk.text
          ) : (
            <span title={risk.title} className="underline decoration-dotted underline-offset-2">
              {risk.text}
            </span>
          )}
          {index < risks.length - 1 && " · "}
        </span>
      ))}
    </p>
  )
}
