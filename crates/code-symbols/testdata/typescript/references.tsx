// Phantom in a comment
/** Ghost in a doc comment */
export function Panel({ title, items = DEFAULT_ITEMS }: PanelProps) {
  const [open, setOpen] = useState(false)
  const label = `Phantom ${title} ${items.length}`
  const handler = () => setOpen(!open) || later()
  for (const item of items) {
    track(item)
  }
  for (index of indexes) {
    track(index)
  }
  try {
    render(label)
  } catch (error) {
    report(error)
  }
  const options = { title, mode: Mode.Dark, [key]: value, method() { return this.mode } }
  var hoisted = compute(handler)
  return (
    <Card title={title} onClick={handler}>
      <div className="row">{label}</div>
      <Card.Footer open={open} />
    </Card>
  )
  function later() {
    return hoisted
  }
}
