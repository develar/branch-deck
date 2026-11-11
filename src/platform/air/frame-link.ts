/**
 * The frame link of a view of the IDE (docs/design/ide-embedding/protocol/README.md, "The links"): MCP Apps of SEP-1865
 * with the extension `x-air-app`, JSON-RPC 2.0 by postMessage between the frame of the page and the host page of the IDE.
 * The page asks (`tools/call`, `x-air-app/subscribe`), the host answers each request by its id and tells the page its
 * notifications in the order of its WebSocket (`x-air-app/action`, `x-air-app/callEvent`, `notifications/progress`)
 */

/** The error of a request, as the host answers it: a code of JSON-RPC and the reason of the contract in `data` */
export interface LinkError {
  readonly code: number
  readonly message: string
  readonly data?: { readonly reason?: string } & Record<string, unknown>
}

/** A request the host refused or the link lost: the error of its answer */
export class FrameLinkError extends Error {
  readonly code: number
  readonly reason: string | null

  constructor(readonly error: LinkError) {
    super(error.message)
    this.name = "FrameLinkError"
    this.code = error.code
    this.reason = error.data?.reason ?? null
  }
}

/** Where the link posts and what it hears: the parent of the frame in the IDE, a fake in a unit test */
export interface FrameEndpoint {
  post(message: unknown): void
  /** Calls `listener` with each message of the host; the function it answers stops that */
  listen(listener: (message: unknown) => void): () => void
}

/** The parent window of the frame: only its messages are the host's */
export function parentEndpoint(target: Window = window): FrameEndpoint {
  return {
    post: message => target.parent.postMessage(message, "*"),
    listen: listener => {
      const onMessage = (event: MessageEvent) => {
        if (event.source === target.parent) {
          listener(event.data)
        }
      }
      target.addEventListener("message", onMessage)
      return () => target.removeEventListener("message", onMessage)
    },
  }
}

type Params = Record<string, unknown>

interface Pending {
  readonly resolve: (result: unknown) => void
  readonly reject: (error: Error) => void
}

/** A JSON-RPC 2.0 client of the host over an endpoint */
export class FrameLink {
  private nextId = 1
  private readonly pending = new Map<number, Pending>()
  private readonly handlers = new Map<string, (params: Params) => void>()

  constructor(private readonly endpoint: FrameEndpoint) {
    endpoint.listen(message => this.receive(message))
  }

  /** The id the next request takes: a caller that needs it before the answer (the `callId` of the events) reads it */
  peekId(): number {
    return this.nextId
  }

  /** A request; resolves with its result, rejects with a `FrameLinkError` of its error */
  request<T = unknown>(method: string, params: Params = {}): Promise<T> {
    const id = this.nextId++
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (result: unknown) => void, reject })
      this.endpoint.post({ jsonrpc: "2.0", id, method, params })
    })
  }

  notify(method: string, params: Params = {}): void {
    this.endpoint.post({ jsonrpc: "2.0", method, params })
  }

  /** The handler of the notification `method` of the host, one per method */
  on(method: string, handler: (params: Params) => void): void {
    this.handlers.set(method, handler)
  }

  private receive(message: unknown): void {
    if (message === null || typeof message !== "object" || (message as { jsonrpc?: unknown }).jsonrpc !== "2.0") {
      return
    }
    const { id, method, params, result, error } = message as { id?: unknown; method?: unknown; params?: Params; result?: unknown; error?: LinkError }
    if (typeof method === "string") {
      // the host asks the frame nothing but a ping (and the teardown of SEP-1865, which the IDE drives by the place)
      if (id !== undefined) {
        this.endpoint.post(
          method === "ping" || method === "ui/resource-teardown"
            ? { jsonrpc: "2.0", id, result: {} }
            : { jsonrpc: "2.0", id, error: { code: -32601, message: `${method} is not served by the page` } },
        )
        return
      }
      this.handlers.get(method)?.(params ?? {})
      return
    }
    const pending = typeof id === "number" ? this.pending.get(id) : undefined
    if (pending === undefined) {
      return
    }
    this.pending.delete(id as number)
    if (error !== undefined) {
      pending.reject(new FrameLinkError(error))
    } else {
      pending.resolve(result)
    }
  }
}
