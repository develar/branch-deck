import type { ReactNode } from "react"
import type { LucideIcon } from "lucide-react"
import { cn } from "cn"

export interface InfoCardProps {
  title: string
  icon: LucideIcon
  iconClassName?: string
  children: ReactNode
}

export function InfoCard({ title, icon: Icon, iconClassName = "text-info", children }: InfoCardProps) {
  return (
    <div className="flex flex-col gap-3 rounded-lg bg-muted bd-padding-card">
      <div className="flex items-center gap-2">
        <Icon className={cn("size-4 shrink-0", iconClassName)} />
        <h3 className="text-sm font-medium break-normal text-foreground">{title}</h3>
      </div>
      <div className="text-sm break-normal text-muted-foreground">{children}</div>
    </div>
  )
}
