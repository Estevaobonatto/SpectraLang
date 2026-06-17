use crate::{read_args, read_spectra_string, write_result};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};

pub const JSON_KIND_INVALID: SpectraHostValue = 0;
pub const JSON_KIND_NULL: SpectraHostValue = 1;
pub const JSON_KIND_BOOL: SpectraHostValue = 2;
pub const JSON_KIND_NUMBER: SpectraHostValue = 3;
pub const JSON_KIND_STRING: SpectraHostValue = 4;
pub const JSON_KIND_ARRAY: SpectraHostValue = 5;
pub const JSON_KIND_OBJECT: SpectraHostValue = 6;

pub extern "C" fn json_validate(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let valid = read_spectra_string(args[0])
        .map(|text| classify_json(&text) != JSON_KIND_INVALID)
        .unwrap_or(false);
    write_result(ctx, valid as SpectraHostValue)
}

pub extern "C" fn json_kind(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let kind = read_spectra_string(args[0])
        .map(|text| classify_json(&text))
        .unwrap_or(JSON_KIND_INVALID);
    write_result(ctx, kind)
}

fn classify_json(text: &str) -> SpectraHostValue {
    let trimmed = text.trim();
    if trimmed == "null" {
        JSON_KIND_NULL
    } else if trimmed == "true" || trimmed == "false" {
        JSON_KIND_BOOL
    } else if is_json_number(trimmed) {
        JSON_KIND_NUMBER
    } else if is_json_string(trimmed) {
        JSON_KIND_STRING
    } else if balanced_container(trimmed, '[', ']') {
        JSON_KIND_ARRAY
    } else if balanced_container(trimmed, '{', '}') {
        JSON_KIND_OBJECT
    } else {
        JSON_KIND_INVALID
    }
}

fn is_json_number(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    text.parse::<f64>().is_ok()
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E'))
}

fn is_json_string(text: &str) -> bool {
    text.len() >= 2 && text.starts_with('"') && text.ends_with('"')
}

fn balanced_container(text: &str, open: char, close: char) -> bool {
    if !(text.starts_with(open) && text.ends_with(close)) {
        return false;
    }
    let mut in_string = false;
    let mut escaped = false;
    let mut depth = 0_i32;
    for ch in text.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0 && !in_string
}
