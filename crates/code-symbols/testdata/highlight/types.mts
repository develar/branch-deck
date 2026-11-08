// A module of TypeScript types: `<T>x` asserts a type in a `.mts` file
export declare function load<T = unknown>(path: string): Promise<T>
export declare const LIMIT: number

type Keys<T> = { [K in keyof T]: T[K] extends Function ? never : K }
type Head<T> = T extends [infer H, ...unknown[]] ? H : never
type Route = `/${string}` | `#${number}`

export interface Store {
  [key: string]: unknown
  pair: [label: string, count: number]
  is(value: unknown): value is Store
  with(this: Store, ...rest: string[]): this
}

const size = <number>load<number>("size")
const { as: alias } = z.infer(size)
