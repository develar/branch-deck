import { cn } from "cn"
import type { PriceRisk } from "@/presenters/landing-plan"

/**
 * The price of Land Together as its risks, each with its whole in the hover: "1 robot instead of 2 · same 7 checks · a
 * failure holds both" (`describeTogetherPrice`); the frame of an offer in the plan and the Safe Push popover show the same
 */
export function PriceLine({ risks, lead, className, testId = "plan-price" }: { risks: readonly PriceRisk[]; lead?: string; className?: string; testId?: string }) {
  return (
    <p className={cn("flex flex-wrap items-center gap-x-1 text-xs text-muted-foreground", className)} data-testid={testId}>
      {lead !== undefined && <span className="text-foreground">{`${lead} `}</span>}
      {risks.map((risk, index) => (
        // the separator ends the risk before it, so a wrapped line starts with a risk
        <span key={risk.text} className="whitespace-nowrap">
          <span title={risk.title} className={cn(risk.warns === true && "text-warning", risk.title !== undefined && "underline decoration-dotted underline-offset-2")}>
            {risk.text}
          </span>
          {index < risks.length - 1 && <span aria-hidden>{" · "}</span>}
        </span>
      ))}
    </p>
  )
}
