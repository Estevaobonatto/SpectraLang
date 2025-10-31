module examples.errors;

fn risky_division(value: i32, divisor: i32): i32 {
    try {
        if (divisor == 0) {
            return 0;
        }
        return value / divisor;
    } catch problem {
        let _captured = problem;
        return -1;
    }
}

fn demo(flag: bool): i32 {
    try {
        if (flag) {
            return risky_division(10, 2);
        }
        return risky_division(10, 0);
    } catch err {
        let _ignored = err;
        return -99;
    }
}

fn main(): i32 {
    return demo(false);
}
