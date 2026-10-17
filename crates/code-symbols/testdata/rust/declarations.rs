//! A module of shapes.

use std::fmt;

/// A shape.
#[derive(Debug, Clone)]
pub struct Circle {
  pub radius: f64,
  label: String,
  pub(crate) scale: u32,
}

pub(crate) struct Pair(pub u8, u8);

pub enum Kind {
  Round,
  Polygon(u8),
  Custom { sides: u8 },
}

pub union Bits {
  int: u32,
  float: f32,
}

pub trait Shape: fmt::Debug {
  type Unit;
  const SIDES: u32;
  fn area(&self) -> f64;
  fn name() -> &'static str {
    "shape"
  }
}

pub type Shapes = Vec<Box<dyn Shape<Unit = f64>>>;
pub const MAX: usize = 10;
static mut COUNTER: u32 = 0;

pub(super) fn parent_only() {}
fn private_helper() {}

macro_rules! square {
  ($x:expr) => {
    $x * $x
  };
}

mod geometry {
  pub fn distance() -> f64 {
    0.0
  }
  fn hidden() {}
}

impl Circle {
  pub fn new(radius: f64) -> Self {
    Self { radius, label: String::new(), scale: 1 }
  }

  fn grow(&mut self) {
    self.radius *= 2.0;
  }

  const UNIT: f64 = 1.0;
}

impl Shape for Circle {
  type Unit = f64;
  const SIDES: u32 = 0;

  fn area(&self) -> f64 {
    square!(self.radius) * std::f64::consts::PI
  }
}

impl fmt::Display for Kind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "kind")
  }
}
