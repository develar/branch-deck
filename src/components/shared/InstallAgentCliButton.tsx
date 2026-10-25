import { useState } from "react"
import { TerminalIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { installAgentCli } from "@/hooks/useInstallAgentCli"

/**
 * "Install Agent CLI": `bd install` and its toast, as Help → Install Agent CLI…; `openAgentSettings`: the toast opens
 * Settings → Agents, which the main window offers and Settings itself does not need
 */
export function InstallAgentCliButton({ openAgentSettings }: { openAgentSettings: boolean }) {
  const [installing, setInstalling] = useState(false)
  return (
    <Button
      size="xs"
      variant="outline"
      disabled={installing}
      aria-busy={installing || undefined}
      onClick={() => {
        setInstalling(true)
        void installAgentCli(openAgentSettings).finally(() => setInstalling(false))
      }}
    >
      <TerminalIcon data-icon="inline-start" />
      Install Agent CLI
    </Button>
  )
}
