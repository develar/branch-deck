import { BellIcon } from "lucide-react"
import { Container } from "@/components/layout/Container"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { toast, Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"

// Smoke page for the shadcn setup; replaced by the router shell in the next phase
export default function App() {
  return (
    <TooltipProvider>
      <Container data-testid="app-container" className="flex flex-col items-start gap-4 py-6">
        <Badge variant="success">synced</Badge>
        <Button
          onClick={() => {
            toast.add({ title: "Branch Deck", description: "Toast from the shadcn toast manager", type: "success" })
          }}
        >
          <BellIcon data-icon="inline-start" />
          Show toast
        </Button>
      </Container>
      <Toaster />
    </TooltipProvider>
  )
}
