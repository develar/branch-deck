import { useState } from "react"

export function Counter({ start }: { start: number }) {
  const [count, setCount] = useState(start)
  return (
    <div className="counter" data-count={count}>
      <Button onClick={() => setCount(count + 1)}>Add</Button>
      {count > 10 && <span>many</span>}
    </div>
  )
}
