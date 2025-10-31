module app.types_demo;

import lib.types;

fn main(): i32 {
  let start: Point = origin;
  let shifted: Point = translate(start, 3, -2);
  let path = [start, shifted];
  let final_point: Point = path[1];
  return final_point.x + final_point.y;
}
