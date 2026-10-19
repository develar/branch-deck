//! A crate of boxes
use std::collections::HashMap;
use crate::model::{Item, Kind::Small};

/// A box of `T`
#[derive(Debug, Clone)]
pub struct Box<'a, T: Clone> {
  pub items: Vec<T>,
  name: &'a str,
}

pub const LIMIT: usize = 10;
static mut CREATED: u32 = 0;

pub enum Kind { Small, Large(u8) }

impl<'a, T: Clone> Box<'a, T> {
  pub fn add(&mut self, item: T, times: usize) -> bool {
    let mut count = 0;
    let text = format!("x={} \"q\"\n", self.name);
    'outer: for i in 0..times {
      if i > LIMIT { break 'outer; }
      self.items.push(item.clone());
      count += 1;
    }
    let point = Point { x: 1, y: 2.5 };
    let Point { x, .. } = point;
    println!("{text} {count} {x} {}", b'c' as u32 + 0x1F);
    matches!(Kind::Small, Small) && !self.items.is_empty() && true
  }
}

macro_rules! twice { ($e:expr) => { $e * 2 }; }
