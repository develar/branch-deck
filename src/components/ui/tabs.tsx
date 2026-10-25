// local customisation: `orientation` reaches Base UI's Root (it only wrote `data-orientation`, so the arrows of a vertical
// list stayed ←/→); the `list` variant of `TabsList`, a sidebar on the canvas whose selected tab is the selected row of
// the tables (`bg-primary/15`), and `bg-primary/25` while the list has the focus, the active selection of an IDE list, without
// a focus ring (with `activateOnFocus` the focus is always on the selected tab, and a window that opens focuses it); no text
// size (the container sets it); no `bg-input` fill on the active tab of dark
import { Tabs as TabsPrimitive } from "@base-ui/react/tabs"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"

function Tabs({ className, orientation = "horizontal", ...props }: TabsPrimitive.Root.Props) {
  return <TabsPrimitive.Root data-slot="tabs" orientation={orientation} className={cn("group/tabs flex gap-2 data-horizontal:flex-col", className)} {...props} />
}

const tabsListVariants = cva(
  "group/tabs-list inline-flex w-fit items-center justify-center rounded-lg p-[3px] text-muted-foreground group-data-horizontal/tabs:h-8 group-data-vertical/tabs:h-fit group-data-vertical/tabs:flex-col data-[variant=line]:rounded-none",
  {
    variants: {
      variant: {
        default: "bg-muted",
        line: "gap-1 bg-transparent",
        list: "w-full items-stretch gap-0.5 bg-transparent group-data-vertical/tabs:p-0",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
)

function TabsList({ className, variant = "default", ...props }: TabsPrimitive.List.Props & VariantProps<typeof tabsListVariants>) {
  return <TabsPrimitive.List data-slot="tabs-list" data-variant={variant} className={cn(tabsListVariants({ variant }), className)} {...props} />
}

function TabsTrigger({ className, ...props }: TabsPrimitive.Tab.Props) {
  return (
    <TabsPrimitive.Tab
      data-slot="tabs-trigger"
      className={cn(
        "relative inline-flex h-[calc(100%-1px)] flex-1 items-center justify-center gap-1.5 rounded-md border border-transparent px-1.5 py-0.5 font-medium whitespace-nowrap text-foreground/60 transition-all group-data-vertical/tabs:w-full group-data-vertical/tabs:justify-start group-data-vertical/tabs:py-[calc(--spacing(1.25))] hover:text-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring disabled:pointer-events-none disabled:opacity-50 has-data-[icon=inline-end]:pr-1 has-data-[icon=inline-start]:pl-1 aria-disabled:pointer-events-none aria-disabled:opacity-50 dark:text-muted-foreground dark:hover:text-foreground [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5",
        "group-data-[variant=line]/tabs-list:bg-transparent group-data-[variant=line]/tabs-list:data-active:bg-transparent dark:group-data-[variant=line]/tabs-list:data-active:border-transparent dark:group-data-[variant=line]/tabs-list:data-active:bg-transparent",
        "data-active:bg-background data-active:text-foreground dark:data-active:border-input dark:data-active:text-foreground",
        "group-data-[variant=list]/tabs-list:h-7 group-data-[variant=list]/tabs-list:flex-none group-data-[variant=list]/tabs-list:rounded-lg group-data-[variant=list]/tabs-list:px-2 group-data-[variant=list]/tabs-list:font-normal group-data-[variant=list]/tabs-list:text-foreground group-data-[variant=list]/tabs-list:hover:bg-muted group-data-[variant=list]/tabs-list:focus-visible:border-transparent group-data-[variant=list]/tabs-list:focus-visible:ring-0 group-data-[variant=list]/tabs-list:focus-visible:outline-none group-data-[variant=list]/tabs-list:data-active:bg-primary/15 group-data-[variant=list]/tabs-list:data-active:hover:bg-primary/20 group-data-[variant=list]/tabs-list:data-active:focus:bg-primary/25",
        "after:absolute after:bg-foreground after:opacity-0 after:transition-opacity group-data-horizontal/tabs:after:inset-x-0 group-data-horizontal/tabs:after:bottom-[-5px] group-data-horizontal/tabs:after:h-0.5 group-data-vertical/tabs:after:inset-y-0 group-data-vertical/tabs:after:-right-1 group-data-vertical/tabs:after:w-0.5 group-data-[variant=line]/tabs-list:data-active:after:opacity-100",
        className,
      )}
      {...props}
    />
  )
}

function TabsContent({ className, ...props }: TabsPrimitive.Panel.Props) {
  return <TabsPrimitive.Panel data-slot="tabs-content" className={cn("flex-1 outline-none", className)} {...props} />
}

export { Tabs, TabsList, TabsTrigger, TabsContent, tabsListVariants }
