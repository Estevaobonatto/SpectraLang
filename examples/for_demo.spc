module demo.for_loops;

fn calculate_sum(): i32 {
    var result = 0;
    
    for (let i = 0; i < 10; i + 1) {
        result = result + i;
    }
    
    return result;
}

fn main(): i32 {
    var total = 0;
    
    for (var counter = 1; counter <= 5; counter + 1) {
        if (counter == 3) {
            continue;
        }
        total = total + counter;
        if (counter == 4) {
            break;
        }
    }
    
    return total;
}
