// The headers a summary writes, the text `trial-check` compares and the review shows (`as_written`)
export function generic<T extends object = {}, K extends keyof T = keyof T>(value: T, key: K): T[K] {
  return value[key]
}

export async function* stream(source: AsyncIterable<string>, limit = 10): AsyncGenerator<string, void, unknown> {
  for await (const item of source) yield item.slice(0, limit)
}

export function overloaded(value: string): string
export function overloaded(value: number): number
export function overloaded(value: string | number): string | number {
  return value
}

export const arrow = <T,>(items: readonly T[], predicate: (item: T, index: number) => boolean): T | undefined => items.find(predicate)

export const template = `a ${"b"} c`
export const literal = { "quoted-key": 1, nested: { deep: [1, 2] } } as const

export class Store<State extends Record<string, unknown>> {
  static readonly empty: Store<{}> = new Store({})
  #listeners = new Set<(state: State) => void>()

  constructor(private state: State, readonly name?: string) {}

  get current(): Readonly<State> {
    return this.state
  }

  set current(next: Readonly<State>) {
    this.state = { ...next }
  }

  subscribe(listener: (state: State) => void): () => void {
    this.#listeners.add(listener)
    return () => this.#listeners.delete(listener)
  }

  protected abstractLike?(): void
}

export interface Shape<T = number> {
  readonly area: (scale?: T) => T
  [key: string]: unknown
  new (size: T): Shape<T>
}

export type Mapped<T> = { readonly [K in keyof T]?: T[K] extends Function ? never : T[K] }
export enum Mode { Plain = "plain", Rich = 1 << 2 }
declare function ambient(input: unknown): asserts input is string
