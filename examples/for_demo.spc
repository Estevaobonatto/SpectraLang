module demo.loops;

fn sum_to(limit: i32): i32 {
    var total = 0;
    var index = 0;
    while (index < limit) {
        total = total + index;
        index = index + 1;
    }
    return total;
}

fn main(): i32 {
    var total = 0;
    for (let i = 1; i <= 5; i + 1) {
        if (i == 3) {
            continue;
        }
        total = total + i;
        if (i == 4) {
            break;
        }
    }
    return total + sum_to(5);
}
