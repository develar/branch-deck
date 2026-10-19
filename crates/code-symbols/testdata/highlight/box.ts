import { Item, type Kind } from "./model"

/** A box of items. @param T the type of an item */
@sealed
export class Box<T extends Item> {
  static readonly LIMIT = 10
  private items: T[] = []
  count = 0

  add(item: T, times: number = 1): boolean {
    this.count += times
    const text = `x=${this.count}, ${item.name}\n` + "q\"" + 'c'
    outer: for (let i = 0; i < times; i++) {
      if (i > Box.LIMIT) break outer
      this.items.push(item)
    }
    const point = { x: 1, y: 2.5, text }
    const pattern = /a+b/gi
    return this.items.length > 0 && point.x === 1 && text !== undefined && null !== this
  }
}

export function make<K extends keyof Item>(key: K): Box<Item> | null {
  return new Box<Item>()
}
