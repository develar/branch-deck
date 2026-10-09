import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { formatTimestamp } from "@/lib/time"

export interface TimeWithPopoverProps {
  authorTime: number
  committerTime?: number
}

/** Author time; if the commit time differs, hovering shows both. */
export function TimeWithPopover({ authorTime, committerTime }: TimeWithPopoverProps) {
  if (!committerTime || authorTime === committerTime) {
    return <span>{formatTimestamp(authorTime)}</span>
  }

  return (
    <Popover>
      <PopoverTrigger openOnHover nativeButton={false} render={<span />}>
        {formatTimestamp(authorTime)}
      </PopoverTrigger>
      <PopoverContent className="w-auto p-3">
        <div className="grid grid-cols-[auto_1fr] gap-x-2 gap-y-1 text-xs">
          <div className="text-muted-foreground">Authored</div>
          <div className="text-foreground">{formatTimestamp(authorTime)}</div>
          <div className="text-muted-foreground">Committed</div>
          <div className="text-foreground">{formatTimestamp(committerTime)}</div>
        </div>
      </PopoverContent>
    </Popover>
  )
}
