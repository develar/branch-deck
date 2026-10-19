import { HighlightEngine } from "@/lib/diff/highlight/engine"
import { createGrammarSource } from "@/lib/diff/highlight/grammars"
import type { HighlightRequest } from "@/lib/diff/highlight/protocol"

// The highlight worker (docs/design/diff-view.md, "Syntax highlighting"): the requests that come before the grammars
// are ready wait for them

const waiting: HighlightRequest[] = []
let engine: HighlightEngine | undefined

self.addEventListener("message", (event: MessageEvent<HighlightRequest>) => {
  if (engine === undefined) {
    waiting.push(event.data)
  } else {
    engine.handle(event.data)
  }
})

void createGrammarSource().then(source => {
  engine = new HighlightEngine(source, (answer, transfer) => self.postMessage(answer, { transfer }))
  for (const request of waiting.splice(0)) {
    engine.handle(request)
  }
})
