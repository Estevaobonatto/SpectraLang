module demo.fib;

fn fib_at(n: i32): i32 {
    var table = [0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
    var index = 2;
    while (index <= n) {
        table[index] = table[index - 1] + table[index - 2];
        index = index + 1;
    }
    return table[n];
}

fn main(): i32 {
    return fib_at(7);
}
