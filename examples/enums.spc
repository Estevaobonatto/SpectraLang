module demo.enums;

enum Message {
    Quit,
    Write(string),
    Move(i32, i32),
    Resize { width: i32, height: i32 }
}

fn send_default(): Message {
    return Message::Quit;
}

fn build_move(x: i32, y: i32): Message {
    return Message::Move(x, y);
}

fn build_resize(width: i32, height: i32): Message {
    return Message::Resize { width: width, height: height };
}

fn main(): i32 {
    let basic = send_default();
    let action = build_move(4, 5);
    let resize = build_resize(640, 480);
    let _basic = basic;
    let _action = action;
    let _resize = resize;
    return 0;
}
