import { useEffect, useRef } from "react"
import { listen } from "@tauri-apps/api/event"
import type { Event, EventName, UnlistenFn } from "@tauri-apps/api/event"
import { logError } from "@/lib/errors"

/**
 * Subscribes to an async listener (Tauri event, window event, menu event) for the lifetime of the component.
 * `subscribe` runs once per mount (and again only if `name` changes); it may be an inline closure.
 * StrictMode-safe: if the component unmounts before the subscription resolves, it is released immediately.
 */
export function useTauriCustomListen(name: string, subscribe: () => Promise<UnlistenFn>): void {
  const subscribeRef = useRef(subscribe)
  useEffect(() => {
    subscribeRef.current = subscribe
  })

  useEffect(() => {
    let cancelled = false
    let unlisten: UnlistenFn | null = null

    subscribeRef.current()
      .then((fn) => {
        if (cancelled) {
          fn()
        }
        else {
          unlisten = fn
        }
      })
      .catch((error: unknown) => {
        logError(`Failed to listen to event ${name}`, error)
      })

    return () => {
      cancelled = true
      if (unlisten !== null) {
        unlisten()
        unlisten = null
      }
    }
  }, [name])
}

/**
 * Listens to a Tauri event for the lifetime of the component.
 * The latest `handler` is always invoked, so it does not need to be memoized.
 */
export function useTauriListen<T>(event: EventName, handler: (payload: T, event: Event<T>) => void): void {
  const handlerRef = useRef(handler)
  useEffect(() => {
    handlerRef.current = handler
  })

  useTauriCustomListen(event, () => listen<T>(event, (it) => {
    handlerRef.current(it.payload, it)
  }))
}
