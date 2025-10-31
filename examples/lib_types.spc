module lib.types;

pub struct Point {
    x: i32,
    y: i32
}

pub enum Flag {
    On,
    Off
}

pub let origin: Point = Point { x: 0, y: 0 };

pub fn translate(point: Point, dx: i32, dy: i32): Point {
    let shifted_x: i32 = point.x + dx;
    let shifted_y: i32 = point.y + dy;
    return Point { x: shifted_x, y: shifted_y };
}

pub fn mirror(point: Point): Point {
    return Point { x: point.x, y: point.y };
}
