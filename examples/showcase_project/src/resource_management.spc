module showcase.resources;

fn acquire_counter(): i32 {
    return 1;
}

fn compute_total(): i32 {
    using counter: i32 = acquire_counter();
    var total = counter;
    defer {
        total = total - 1;
    }
    total = total + 10;
    return total;
}

fn main(): i32 {
    return compute_total();
}
