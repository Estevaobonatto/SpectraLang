use std::fmt::{self, Display};

/// Represents a position (1-based) in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl Location {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Byte-range span and the corresponding source locations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}
