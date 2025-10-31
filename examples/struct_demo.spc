module demo.structs;

struct Point {
    x: i32,
    y: i32
}

fn translate(point: Point, dx: i32, dy: i32): Point {
    return Point { x: point.x + dx, y: point.y + dy };
}

fn main(): i32 {
    let start = Point { x: 1, y: 2 };
    var current = translate(start, 2, 3);
    current = translate(current, -1, 0);
    return current.x + current.y;
}
