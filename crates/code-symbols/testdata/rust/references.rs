/// Doc mentions Phantom and [`Ghost`].
pub fn render(shapes: &[Shape], scale: u32) -> Result<(), Error> {
  // Phantom in a comment
  /* Ghost in a block */
  let title = format!("Phantom {} of {scale}", shapes.len());
  let count = shapes.len() * scale as usize;
  for (index, shape) in shapes.iter().enumerate() {
    canvas::draw(shape, index, count);
  }
  let areas: Vec<f64> = shapes.iter().map(|shape| shape.area()).filter(|area| *area > THRESHOLD).collect();
  if let Some(first) = shapes.first() {
    log::info!("first {first:?} {}", first.name());
  }
  match shapes.len() {
    0 => println!("{title}"),
    n if n > MAX => return Err(Error::TooMany(n)),
    _ => {}
  }
  let Circle { radius, scale: circle_scale } = Circle::default();
  let Some(last) = shapes.last() else { return Ok(()) };
  while let Some(next) = queue.pop() {
    next.render(radius, circle_scale, last);
  }
  'outer: loop {
    break 'outer;
  }
  matches!(title.as_str(), crate::NAME);
  Ok(())
}

const THRESHOLD: f64 = 0.5;
