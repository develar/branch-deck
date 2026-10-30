import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"

/** One item of an `ItemSelect` */
export interface SelectOption<T extends string> {
  value: T
  label: string
}

/** A small select of one value of `items`, its popup below the trigger */
export function ItemSelect<T extends string>({
  items,
  value,
  onChange,
  disabled = false,
  className,
  testId,
  placeholder,
  ariaLabel,
  align = "center",
}: {
  items: readonly SelectOption<T>[]
  value: T | null
  onChange: (value: T) => void
  disabled?: boolean
  className?: string
  testId: string
  placeholder?: string
  ariaLabel?: string
  align?: "start" | "center" | "end"
}) {
  return (
    <Select items={items} value={value} disabled={disabled} onValueChange={next => next !== null && onChange(next)}>
      <SelectTrigger size="sm" className={className} aria-label={ariaLabel} data-testid={testId}>
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent alignItemWithTrigger={false} align={align}>
        {items.map(item => (
          <SelectItem key={item.value} value={item.value}>
            {item.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
