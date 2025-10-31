module app.api;

import app.math;

export app.math::add;

pub fn sum_three(a: i32, b: i32, c: i32): i32 {
  let ab = add(a, b);
  return add(ab, c);
}
