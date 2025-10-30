// Enum examples for SpectraLang

// Simple enum with unit variants
enum Color {
    Red,
    Green,
    Blue,
}

// Enum with tuple variants
enum Point {
    TwoD(i32, i32),
    ThreeD(i32, i32, i32),
}

// Enum with struct variants
enum Shape {
    Circle { radius: f32 },
    Rectangle { width: f32, height: f32 },
    Triangle { base: f32, height: f32 },
}

// Mixed enum
enum Message {
    Quit,
    Move { x: i32, y: i32 },
    Write(string),
    ChangeColor(i32, i32, i32),
}

fn main() -> i32 {
    return 0;
}
