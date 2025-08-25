/**
 * A reactive collection that maintains both a Map (for O(1) lookups)
 * and an Array (for UI binding), following the proven pattern from branchSyncProvider
 */
interface KeyedItem {
  name: string
}

export class ReactiveIndexedCollection<K, V extends KeyedItem> {
  private map: Map<K, V>
  private readonly arrayRef: Ref<V[]>

  constructor() {
    this.map = new Map()
    this.arrayRef = ref([])
  }

  get array() {
    return this.arrayRef
  }

  get size() {
    return this.map.size
  }

  // Get item by key
  get(key: K): V | undefined {
    return this.map.get(key)
  }

  // Check if key exists
  has(key: K): boolean {
    return this.map.has(key)
  }

  // Add or update item
  set(key: K, value: V): void {
    const existing = this.map.get(key)
    if (!existing) {
      // Add to both map and array
      this.map.set(key, value)
      this.arrayRef.value.push(value)
    }
    else {
      // Item already exists, just update the map
      // (the reactive object will update automatically)
      this.map.set(key, value)
    }
  }

  // Remove item by key
  delete(key: K): boolean {
    const item = this.map.get(key)
    if (!item) {
      return false
    }

    // Remove from map
    this.map.delete(key)

    // Remove from array using splice (preserve array identity)
    const index = this.arrayRef.value.indexOf(item)
    if (index > -1) {
      this.arrayRef.value.splice(index, 1)
    }

    return true
  }

  // Clear all items
  clear(): void {
    this.map.clear()
    this.arrayRef.value.length = 0 // Clear array without replacing ref
  }

  // Reconcile collection so that the array order matches the provided key order exactly.
  // Reuses existing reactive instances when present and creates new ones when missing.
  // Items not present in newKeys are removed from both the map and array.
  reconcile<TData = unknown>(
    newKeys: Set<K>,
    createFn: (key: K) => V,
    updateFn?: (key: K, existing: V, data?: TData) => void,
    dataMap?: Map<K, TData>,
  ): void {
    // Build a new array following backend key order precisely
    const newArray: V[] = []
    const seen = new Set<K>()

    for (const key of newKeys) {
      const existing = this.map.get(key)
      if (existing) {
        if (updateFn) {
          updateFn(key, existing, dataMap?.get(key))
        }
        newArray.push(existing)
      }
      else {
        const created = createFn(key)
        this.map.set(key, created)
        newArray.push(created)
      }
      seen.add(key)
    }

    // Remove keys not present anymore from the map
    for (const key of Array.from(this.map.keys())) {
      if (!seen.has(key)) {
        this.map.delete(key)
      }
    }

    // Replace array with the newly ordered array in one operation
    this.arrayRef.value = newArray
  }

  // Helper to extract key from item (override in subclass if needed)
  protected getKeyFromItem(item: V): K {
    // Default assumes 'name' property, override as needed
    return item.name as K
  }
}

// Factory function for cleaner API
export function createReactiveIndexedCollection<K, V extends KeyedItem>() {
  return new ReactiveIndexedCollection<K, V>()
}