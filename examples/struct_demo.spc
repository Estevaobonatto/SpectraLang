module demo.structs;

struct Point {
    x: i32,
    y: i32
}

fn main(): i32 {
    var total = 0;
    for (let i = 0; i < 5; i + 1) {
        total = total + i;
    }
    
    return total;
}
