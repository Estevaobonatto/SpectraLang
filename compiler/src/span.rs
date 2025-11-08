#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl Location {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub start_location: Location,
    pub end_location: Location,
}

impl Span {
    pub const fn new(
        start: usize,
        end: usize,
        start_location: Location,
        end_location: Location,
    ) -> Self {
        Self {
            start,
            end,
            start_location,
            end_location,
        }
    }

    pub const fn dummy() -> Self {
        Self {
            start: 0,
            end: 0,
            start_location: Location::new(1, 1),
            end_location: Location::new(1, 1),
        }
    }
}

pub fn span_union(left: Span, right: Span) -> Span {
    Span {
        start: left.start.min(right.start),
        end: left.end.max(right.end),
        start_location: if left.start <= right.start {
            left.start_location
        } else {
            right.start_location
        },
        end_location: if left.end >= right.end {
            left.end_location
        } else {
            right.end_location
        },
    }
}
