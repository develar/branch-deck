import { describe, expect, it } from "vite-plus/test"
import { cutList, joinNames } from "@/lib/text"

describe("joinNames", () => {
  it("names up to four, so 'and 1 more' never stands where one name would, and counts the rest from five on", () => {
    expect([[], ["a"], ["a", "b"], ["a", "b", "c"], ["a", "b", "c", "d"]].map(names => joinNames(names))).toEqual(["", "a", "a and b", "a, b and c", "a, b, c and d"])
    expect(joinNames(["a", "b", "c", "d", "e"])).toBe("a, b, c and 2 more")
    expect(joinNames(["a", "b", "c", "d", "e", "f", "g"])).toBe("a, b, c and 4 more")
  })
})

describe("cutList", () => {
  it("keeps a list one longer than its limit whole and cuts a longer one at the limit, counting the rest of all", () => {
    expect(cutList([1, 2, 3], 3)).toEqual({ shown: [1, 2, 3], more: 0 })
    // the one item costs no more than its count
    expect(cutList([1, 2, 3, 4], 3)).toEqual({ shown: [1, 2, 3, 4], more: 0 })
    expect(cutList([1, 2, 3, 4, 5], 3)).toEqual({ shown: [1, 2, 3], more: 2 })
    // the first items of a list the backend cut: the rest of all of them
    expect(cutList([1, 2, 3, 4], 20, 60)).toEqual({ shown: [1, 2, 3, 4], more: 56 })
    expect(cutList([], 3)).toEqual({ shown: [], more: 0 })
  })
})
