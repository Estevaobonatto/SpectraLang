use crate::{
    ast::TypeAnnotation,
    span::span_union,
};

use super::Parser;

impl Parser {
    pub(super) fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, ()> {
        // Parse type path like: Vec, std.collections.HashMap, etc.
        let start_span = self.current().span;
        
        let (first_segment, _) = self.consume_identifier("Expected type name")?;
        let mut segments = vec![first_segment];

        // Handle qualified types (e.g., std.collections.HashMap)
        while self.check_symbol('.') {
            self.advance(); // consume '.'
            let (segment, _) = self.consume_identifier("Expected identifier after '.'")?;
            segments.push(segment);
        }

        let end_span = self.tokens.get(self.position.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(start_span);

        Ok(TypeAnnotation {
            segments,
            span: span_union(start_span, end_span),
        })
    }
}
