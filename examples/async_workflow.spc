module examples.async;

async fn fetch_seed(): i32 {
    return 7;
}

async fn compute_score(factor: i32): i32 {
    let seed = await fetch_seed();
    return seed * factor;
}

async fn orchestrate(): i32 {
    let left = await compute_score(2);
    let right = await compute_score(3);
    return left + right;
}

fn main(): i32 {
    let _operation = orchestrate();
    return 0;
}
