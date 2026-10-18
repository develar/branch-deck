import { describe, expect, it } from "vite-plus/test"
import { joinNames } from "@/lib/text"

describe("joinNames", () => {
  it("names up to four, so 'and 1 more' never stands where one name would, and counts the rest from five on", () => {
    expect([[], ["a"], ["a", "b"], ["a", "b", "c"], ["a", "b", "c", "d"]].map(names => joinNames(names))).toEqual(["", "a", "a and b", "a, b and c", "a, b, c and d"])
    expect(joinNames(["a", "b", "c", "d", "e"])).toBe("a, b, c and 2 more")
    expect(joinNames(["a", "b", "c", "d", "e", "f", "g"])).toBe("a, b, c and 4 more")
  })
})
