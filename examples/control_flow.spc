module demo.control;

fn describe_grade(score: i32): string {
    if (score >= 90) {
        return "Excellent";
    } elif (score >= 75) {
        return "Good";
    } else {
        return "Needs Review";
    }
}

fn describe_weather(temp: i32): string {
    cond {
        temp >= 90 => return "Hot";
        temp >= 70 => return "Warm";
        temp >= 50 => return "Mild";
        else => return "Cold";
    }
}

fn access_level(is_admin: bool, has_override: bool): string {
    unless (is_admin) {
        if (has_override) {
            return "Limited Access";
        }
        return "Access Denied";
    } elif (has_override) {
        return "Override Granted";
    } else {
        return "Full Access";
    }
}

fn status_label(code: i32): string {
    switch (code) {
        case 200 => return "OK";
        case 400 => return "Bad Request";
        case 500 => return "Server Error";
        default => return "Unknown";
    }
}

fn main(): i32 {
    let _grade = describe_grade(82);
    let _weather = describe_weather(72);
    let _restricted = access_level(false, true);
    let _admin = access_level(true, false);
    let _status = status_label(200);
    return 0;
}
