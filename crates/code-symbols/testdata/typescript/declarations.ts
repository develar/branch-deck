/** A shape. */
export interface Shape {
  area(): number
  readonly name: string
}

export type Props = { onClick: () => void; label: string }
type Internal = Partial<Props>

export abstract class Base implements Shape {
  abstract area(): number
  readonly name = "base"
}

export default class Circle extends Base {
  static unit = 1
  private secret = 0
  #hidden = 1
  protected guarded = 2

  constructor(private readonly radius: number, public label: string) {
    super()
  }

  area(): number {
    return Math.PI * this.radius ** 2
  }

  get diameter() {
    return this.radius * 2
  }
}

export enum Kind {
  Round,
  Square = 4,
}

export namespace Geometry {
  export const ORIGIN = 0
  const hidden = 1
}

declare module "shapes-extra" {
  export function extra(): void
}

export function makeCircle(radius: number): Circle {
  return new Circle(radius, "c")
}

export const MAX_SIZE = 10
export const createShape = (size: number) => new Circle(size, "s")
let counter = 0
const { first, second: renamed } = pair()
function helper() {}
export { helper, counter as count }
