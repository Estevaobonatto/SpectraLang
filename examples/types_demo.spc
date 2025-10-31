module demo.types;

struct Point {
    x: i32,
    y: i32
}

fn translate(point: Point, dx: i32, dy: i32): Point {
    return Point { x: point.x + dx, y: point.y + dy };
}

fn mirror(point: Point): Point {
    return Point { x: point.x, y: -point.y };
}

fn main(): i32 {
    let start = Point { x: 0, y: 0 };
    let shifted = translate(start, 3, -2);
    var path = [start, start, start];
    path[1] = shifted;
    path[2] = mirror(shifted);
    let final_point = path[2];
    return final_point.x + final_point.y;
}
