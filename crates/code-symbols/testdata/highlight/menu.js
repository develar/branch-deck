// A menu of JavaScript, its JSX in a `.js` file
import { render } from "./dom"

/** The items of a menu. */
export function Menu({ items, onPick = () => {} }) {
  const label = `${items.length} items\x1f`
  return (
    <>
      <Menu.Item kind="header" onClick={() => onPick(label)}>{label}</Menu.Item>
      {items.map(item => <li key={item.id}>{item.name}</li>)}
    </>
  )
}

render(Menu, document.body)
