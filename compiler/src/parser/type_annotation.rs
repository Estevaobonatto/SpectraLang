use crate::{
    ast::{TypeAnnotation, TypeAnnotationKind},
    span::span_union,
};

use super::Parser;

impl Parser {
    pub(super) fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, ()> {
        let start_span = self.current().span;

        // Check for Self type
        if self.check_keyword(crate::token::Keyword::SelfType) {
            let end_span = self.current().span;
            self.advance();
            return Ok(TypeAnnotation {
                kind: TypeAnnotationKind::Simple {
                    segments: vec!["Self".to_string()],
                },
                span: span_union(start_span, end_span),
            });
        }

        // Check if it's a tuple type: (int, string, ...)
        if self.check_symbol('(') {
            self.advance(); // consume '('

            let mut elements = Vec::new();

            // Empty tuple type: ()
            if self.check_symbol(')') {
                let end_span = self.current().span;
                self.advance();
                return Ok(TypeAnnotation {
                    kind: TypeAnnotationKind::Tuple { elements },
                    span: span_union(start_span, end_span),
                });
            }

            // Parse first element
            elements.push(self.parse_type_annotation()?);

            // Parse remaining elements
            while self.check_symbol(',') {
                self.advance(); // consume ','

                // Allow trailing comma
                if self.check_symbol(')') {
                    break;
                }

                elements.push(self.parse_type_annotation()?);
            }

            let end_span = self.current().span;
            self.consume_symbol(')', "Expected ')' after tuple type")?;

            return Ok(TypeAnnotation {
                kind: TypeAnnotationKind::Tuple { elements },
                span: span_union(start_span, end_span),
            });
        }

        // Parse simple type path like: Vec, std.collections.HashMap, etc.
        let (first_segment, _) = self.consume_identifier("Expected type name")?;
        let mut segments = vec![first_segment];

        // Handle qualified types (e.g., std.collections.HashMap)
        while self.check_symbol('.') {
            self.advance(); // consume '.'
            let (segment, _) = self.consume_identifier("Expected identifier after '.'")?;
            segments.push(segment);
        }

        let end_span = self
            .tokens
            .get(self.position.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(start_span);

        Ok(TypeAnnotation {
            kind: TypeAnnotationKind::Simple { segments },
            span: span_union(start_span, end_span),
        })
    }
}
