//! FEL hand-rolled recursive descent parser with operator precedence.
//!
//! Chaining multiple `==` / `!=` or multiple comparison operators is a **parse error**; write
//! explicit conjunction (e.g. `0 <= $x and $x <= 10`).
//!
//! Private `parse_*` / `current` / `advance` implement the precedence ladder listed below.
//!
//! `peek().clone()` clones token payloads when inspecting or taking ownership; unavoidable
//! for heap-backed tokens until the lexer borrows source text directly.
#![allow(clippy::missing_docs_in_private_items)]

/// Operator precedence (lowest → highest):
/// 0: let...in, if...then...else
/// 1: ternary ? :
/// 2: or
/// 3: and
/// 4: = !=
/// 5: < > <= >=
/// 6: in, not in
/// 7: ??
/// 8: + - &
/// 9: * / %
/// 10: unary not, unary -
/// 11: postfix . []
use rust_decimal::prelude::*;

use std::collections::HashSet;

use crate::ast::*;
use crate::error::{Error, ParseError};
use crate::lexer::{Lexer, SpannedToken, Token};

/// Recursive-descent parser over a [`SpannedToken`] stream (use [`parse`] to build from source).
pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
    /// When > 0, suppress `in` as membership operator (inside let-value).
    no_in_depth: usize,
    recursion_depth: usize,
    max_recursion_depth: usize,
}

/// Parse a FEL expression string into an AST.
pub fn parse(input: &str) -> Result<Expr, Error> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().map_err(Error::Parse)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        no_in_depth: 0,
        recursion_depth: 0,
        max_recursion_depth: 32,
    };
    let expr = parser.parse_expression()?;
    if !parser.at_eof() {
        return Err(parser.parse_err_current(format!(
            "unexpected token {:?} at position {}",
            parser.current().token,
            parser.current().span.start
        )));
    }
    Ok(expr)
}

impl Parser {
    fn parse_err_token(&self, tok: &SpannedToken, message: impl Into<String>) -> Error {
        Error::Parse(ParseError::with_span(
            tok.span.start..tok.span.end,
            message.into(),
        ))
    }

    fn parse_err_current(&self, message: impl Into<String>) -> Error {
        self.parse_err_token(self.current(), message)
    }

    fn current(&self) -> &SpannedToken {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn peek(&self) -> &Token {
        &self.current().token
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }

    fn advance(&mut self) -> &SpannedToken {
        let tok = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), Error> {
        if self.peek() == expected {
            self.advance();
            Ok(())
        } else {
            Err(self.parse_err_current(format!("expected {expected:?}, got {:?}", self.peek())))
        }
    }

    fn eat_identifier(&mut self) -> Result<String, Error> {
        match self.peek().clone() {
            Token::Identifier(name) => {
                self.advance();
                Ok(name)
            }
            other => Err(self.parse_err_current(format!("expected identifier, got {other:?}"))),
        }
    }

    fn parse_expression_with_in_allowed(&mut self) -> Result<Expr, Error> {
        let saved_no_in = self.no_in_depth;
        self.no_in_depth = 0;
        let result = self.parse_expression();
        self.no_in_depth = saved_no_in;
        result
    }

    // ── Expression (entry point) ────────────────────────────────

    fn parse_expression(&mut self) -> Result<Expr, Error> {
        self.parse_let_or_if()
    }

    fn parse_let_or_if(&mut self) -> Result<Expr, Error> {
        self.recursion_depth += 1;
        if self.recursion_depth > self.max_recursion_depth {
            self.recursion_depth -= 1;
            return Err(self.parse_err_current(format!(
                "expression nesting exceeds maximum depth of {}",
                self.max_recursion_depth
            )));
        }
        let result = self.parse_let_or_if_inner();
        self.recursion_depth -= 1;
        result
    }

    fn parse_let_or_if_inner(&mut self) -> Result<Expr, Error> {
        // let <name> = <value> in <body>
        if matches!(self.peek(), Token::Let) {
            self.advance(); // let
            let name = self.eat_identifier()?;
            self.expect(&Token::Eq)?;
            // Suppress top-level `in` as membership in let-value.
            self.no_in_depth += 1;
            let value = self.parse_ternary()?;
            self.no_in_depth -= 1;
            self.expect(&Token::In)?;
            let body = self.parse_let_or_if()?;
            return Ok(Expr::LetBinding {
                name,
                value: Box::new(value),
                body: Box::new(body),
            });
        }

        // if <cond> then <then> else <else>
        if matches!(self.peek(), Token::If) {
            // Disambiguate: if followed by identifier+( could be function call if()
            // Check for if...then pattern (keyword form)
            if self.is_if_then_else() {
                self.advance(); // if
                let condition = self.parse_ternary()?;
                self.expect(&Token::Then)?;
                let then_branch = self.parse_let_or_if()?;
                self.expect(&Token::Else)?;
                let else_branch = self.parse_let_or_if()?;
                return Ok(Expr::IfThenElse {
                    condition: Box::new(condition),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                });
            }
        }

        self.parse_ternary()
    }

    /// Look ahead to determine if this is `if ... then ... else` (keyword form)
    /// vs `if(...)` (function call form).
    fn is_if_then_else(&self) -> bool {
        let starts_with_paren = matches!(
            self.tokens.get(self.pos + 1).map(|token| &token.token),
            Some(Token::LParen)
        );

        if self.pos + 1 >= self.tokens.len() {
            return false;
        }

        // Scan only this expression. A nested `if(...)` function in an array,
        // object, or argument list must not see the outer keyword form's `then`.
        let mut depth = 0;
        let mut i = self.pos + 1;
        while i < self.tokens.len() {
            match &self.tokens[i].token {
                Token::Then if depth == 0 => return true,
                Token::Comma if starts_with_paren && depth == 1 => return false,
                Token::Comma | Token::RParen | Token::RBracket | Token::RBrace if depth == 0 => {
                    return false;
                }
                Token::LParen | Token::LBracket | Token::LBrace => depth += 1,
                Token::RParen | Token::RBracket | Token::RBrace => {
                    if depth > 0 {
                        depth -= 1;
                    } else {
                        return false;
                    }
                }
                Token::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    // ── Ternary ─────────────────────────────────────────────────

    fn parse_ternary(&mut self) -> Result<Expr, Error> {
        let expr = self.parse_logical_or()?;
        if matches!(self.peek(), Token::Question) {
            self.advance(); // ?
            let then_branch = self.parse_let_or_if()?;
            self.expect(&Token::Colon)?;
            let else_branch = self.parse_let_or_if()?;
            Ok(Expr::Ternary {
                condition: Box::new(expr),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            })
        } else {
            Ok(expr)
        }
    }

    // ── Logical ─────────────────────────────────────────────────

    fn parse_logical_or(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_logical_and()?;
        while matches!(self.peek(), Token::Or) {
            self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::BinaryOp {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_equality()?;
        while matches!(self.peek(), Token::And) {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::BinaryOp {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // ── Equality / Comparison ───────────────────────────────────

    fn parse_equality(&mut self) -> Result<Expr, Error> {
        let left = self.parse_comparison()?;
        let op = match self.peek() {
            Token::Eq => BinaryOp::Eq,
            Token::NotEq => BinaryOp::NotEq,
            _ => return Ok(left),
        };
        self.advance();
        let right = self.parse_comparison()?;
        if matches!(self.peek(), Token::Eq | Token::NotEq) {
            return Err(self.parse_err_current(
                "chained equality is not supported; use explicit logical conjunction",
            ));
        }
        Ok(Expr::BinaryOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    fn parse_comparison(&mut self) -> Result<Expr, Error> {
        let left = self.parse_membership()?;
        let op = match self.peek() {
            Token::Lt => BinaryOp::Lt,
            Token::Gt => BinaryOp::Gt,
            Token::LtEq => BinaryOp::LtEq,
            Token::GtEq => BinaryOp::GtEq,
            _ => return Ok(left),
        };
        self.advance();
        let right = self.parse_membership()?;
        if matches!(
            self.peek(),
            Token::Lt | Token::Gt | Token::LtEq | Token::GtEq
        ) {
            return Err(self.parse_err_current(
                "chained comparisons are not supported; use explicit logical conjunction",
            ));
        }
        Ok(Expr::BinaryOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    // ── Membership ──────────────────────────────────────────────

    fn parse_membership(&mut self) -> Result<Expr, Error> {
        let left = self.parse_null_coalesce()?;

        if self.no_in_depth > 0 {
            return Ok(left);
        }

        // Check for `in` or `not in`
        if matches!(self.peek(), Token::Not) {
            // Peek ahead for `not in`
            if self.pos + 1 < self.tokens.len()
                && matches!(self.tokens[self.pos + 1].token, Token::In)
            {
                self.advance(); // not
                self.advance(); // in
                let right = self.parse_null_coalesce()?;
                return Ok(Expr::Membership {
                    value: Box::new(left),
                    container: Box::new(right),
                    negated: true,
                });
            }
        }

        if matches!(self.peek(), Token::In) {
            self.advance(); // in
            let right = self.parse_null_coalesce()?;
            return Ok(Expr::Membership {
                value: Box::new(left),
                container: Box::new(right),
                negated: false,
            });
        }

        Ok(left)
    }

    // ── Null coalesce ───────────────────────────────────────────

    fn parse_null_coalesce(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_addition()?;
        while matches!(self.peek(), Token::DoubleQuestion) {
            self.advance();
            let right = self.parse_addition()?;
            left = Expr::NullCoalesce {
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // ── Arithmetic ──────────────────────────────────────────────

    fn parse_addition(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_multiplication()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                Token::Ampersand => BinaryOp::Concat,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplication(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinaryOp::Mul,
                Token::Slash => BinaryOp::Div,
                Token::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // ── Unary ───────────────────────────────────────────────────

    fn parse_unary(&mut self) -> Result<Expr, Error> {
        let is_bang = matches!(self.peek(), Token::Bang);
        if matches!(self.peek(), Token::Not | Token::Bang) {
            // Make sure it's not `not in` (handled by membership) — `!` cannot mean `! in`
            if !is_bang
                && self.pos + 1 < self.tokens.len()
                && matches!(self.tokens[self.pos + 1].token, Token::In)
            {
                return self.parse_postfix();
            }
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(operand),
                bang: is_bang,
            });
        }
        if matches!(self.peek(), Token::Minus) {
            // Disambiguate: unary minus vs binary minus
            // Unary if at start of expression or after an operator/opening bracket
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
                bang: false,
            });
        }
        self.parse_postfix()
    }

    // ── Postfix (dot/bracket access) ────────────────────────────

    fn parse_postfix(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_atom()?;
        loop {
            match self.peek() {
                Token::Dot => {
                    self.advance();
                    let name = self.eat_identifier()?;
                    expr = self.attach_postfix_segment(expr, PathSegment::Dot(name));
                }
                Token::LBracket => {
                    self.advance();
                    let seg = self.parse_bracket_segment("")?;
                    expr = self.attach_postfix_segment(expr, seg);
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn attach_postfix_segment(&mut self, expr: Expr, seg: PathSegment) -> Expr {
        Expr::PostfixAccess {
            expr: Box::new(expr),
            path: vec![seg],
        }
    }

    /// Parse `[*]` or `[N]` after the opening `[` has been consumed; consumes `]`.
    fn parse_bracket_segment(&mut self, bracket_context: &str) -> Result<PathSegment, Error> {
        let seg = if matches!(self.peek(), Token::Star) {
            self.advance();
            PathSegment::Wildcard
        } else if let Token::Number(n) = self.peek().clone() {
            let token = self.current().clone();
            if !n.fract().is_zero() || n.is_sign_negative() {
                return Err(self.parse_err_token(&token, format!(
                    "expected non-negative integer index in {bracket_context}brackets, got {n}"
                )));
            }
            let Some(idx_u64) = n.to_u64() else {
                return Err(self.parse_err_token(&token, format!(
                    "index out of range in {bracket_context}brackets, got {n}"
                )));
            };
            let Ok(idx) = usize::try_from(idx_u64) else {
                return Err(self.parse_err_token(&token, format!(
                    "index out of range in {bracket_context}brackets, got {n}"
                )));
            };
            self.advance();
            PathSegment::Index(idx)
        } else {
            return Err(self.parse_err_current(format!(
                "expected number or * in {bracket_context}brackets, got {:?}",
                self.peek()
            )));
        };
        self.expect(&Token::RBracket)?;
        Ok(seg)
    }

    // ── Atoms ───────────────────────────────────────────────────

    fn parse_atom(&mut self) -> Result<Expr, Error> {
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            Token::StringLit(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::String(s))
            }
            Token::True => {
                self.advance();
                Ok(Expr::Boolean(true))
            }
            Token::False => {
                self.advance();
                Ok(Expr::Boolean(false))
            }
            Token::Null => {
                self.advance();
                Ok(Expr::Null)
            }
            Token::DateLiteral(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::DateLiteral(s))
            }
            Token::DateTimeLiteral(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::DateTimeLiteral(s))
            }
            Token::Dollar => {
                self.advance(); // $
                self.parse_field_ref()
            }
            Token::At => {
                self.advance(); // @
                self.parse_context_ref()
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expression_with_in_allowed()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Token::LBracket => self.parse_array_literal(),
            Token::LBrace => self.parse_object_literal(),
            Token::Identifier(name) => {
                let name = name.clone();
                self.advance();
                // Check for function call
                if matches!(self.peek(), Token::LParen) {
                    self.parse_function_call(name)
                } else {
                    // Bare identifier — let-bound variable or unqualified field path
                    Ok(Expr::VarRef { name, path: vec![] })
                }
            }
            Token::If => {
                // if(...) function form
                self.advance();
                if matches!(self.peek(), Token::LParen) {
                    self.parse_function_call("if".to_string())
                } else {
                    Err(self.parse_err_current("unexpected 'if' — use if...then...else or if(...)"))
                }
            }
            _ => Err(self.parse_err_current(format!("unexpected token {:?}", self.peek()))),
        }
    }

    fn parse_field_ref(&mut self) -> Result<Expr, Error> {
        let mut name: Option<String> = None;
        let mut path = Vec::new();

        // Optional identifier after $
        if let Token::Identifier(n) = self.peek().clone() {
            name = Some(n.clone());
            self.advance();
        }

        // Path segments
        loop {
            match self.peek() {
                Token::Dot => {
                    self.advance();
                    let seg_name = self.eat_identifier()?;
                    path.push(PathSegment::Dot(seg_name));
                }
                Token::LBracket => {
                    self.advance();
                    path.push(self.parse_bracket_segment("field ref ")?);
                }
                _ => break,
            }
        }

        Ok(Expr::FieldRef { name, path })
    }

    fn parse_context_ref(&mut self) -> Result<Expr, Error> {
        let name = self.eat_identifier()?;
        let mut arg = None;
        let mut tail = Vec::new();

        // Optional argument: @instance('name')
        if matches!(self.peek(), Token::LParen) {
            self.advance();
            if let Token::StringLit(s) = self.peek().clone() {
                arg = Some(s.clone());
                self.advance();
            }
            self.expect(&Token::RParen)?;
        }

        // Dot-chain: @instance('name').field.subfield
        while matches!(self.peek(), Token::Dot) {
            self.advance();
            tail.push(self.eat_identifier()?);
        }

        Ok(Expr::ContextRef { name, arg, tail })
    }

    fn parse_function_call(&mut self, name: String) -> Result<Expr, Error> {
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        if !matches!(self.peek(), Token::RParen) {
            args.push(self.parse_expression_with_in_allowed()?);
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                args.push(self.parse_expression_with_in_allowed()?);
            }
        }
        self.expect(&Token::RParen)?;
        Ok(Expr::FunctionCall { name, args })
    }

    fn parse_array_literal(&mut self) -> Result<Expr, Error> {
        self.expect(&Token::LBracket)?;
        let mut elements = Vec::new();
        if !matches!(self.peek(), Token::RBracket) {
            elements.push(self.parse_expression_with_in_allowed()?);
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                elements.push(self.parse_expression_with_in_allowed()?);
            }
        }
        self.expect(&Token::RBracket)?;
        Ok(Expr::Array(elements))
    }

    fn parse_object_literal(&mut self) -> Result<Expr, Error> {
        self.expect(&Token::LBrace)?;
        let mut entries = Vec::new();
        let mut seen_keys = HashSet::new();
        if !matches!(self.peek(), Token::RBrace) {
            let entry = self.parse_object_entry()?;
            seen_keys.insert(entry.0.clone());
            entries.push(entry);
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                // Allow trailing comma
                if matches!(self.peek(), Token::RBrace) {
                    break;
                }
                let entry = self.parse_object_entry()?;
                if !seen_keys.insert(entry.0.clone()) {
                    return Err(self.parse_err_current(format!(
                        "duplicate key '{}' in object literal",
                        entry.0
                    )));
                }
                entries.push(entry);
            }
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr::Object(entries))
    }

    fn parse_object_entry(&mut self) -> Result<(String, Expr), Error> {
        let key = match self.peek().clone() {
            Token::Identifier(name) => {
                self.advance();
                name.clone()
            }
            Token::StringLit(s) => {
                self.advance();
                s.clone()
            }
            _ => {
                return Err(
                    self.parse_err_current(format!("expected object key, got {:?}", self.peek()))
                );
            }
        };
        self.expect(&Token::Colon)?;
        let value = self.parse_expression_with_in_allowed()?;
        Ok((key, value))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_docs_in_private_items)]
    use super::*;

    #[test]
    fn test_parse_number() {
        let expr = parse("42").unwrap();
        assert_eq!(expr, Expr::Number(rust_decimal::Decimal::from(42)));
    }

    #[test]
    fn test_parse_string() {
        let expr = parse("\"hello\"").unwrap();
        assert_eq!(expr, Expr::String("hello".into()));
    }

    #[test]
    fn test_parse_boolean() {
        assert_eq!(parse("true").unwrap(), Expr::Boolean(true));
        assert_eq!(parse("false").unwrap(), Expr::Boolean(false));
    }

    #[test]
    fn test_parse_null() {
        assert_eq!(parse("null").unwrap(), Expr::Null);
    }

    #[test]
    fn test_parse_field_ref() {
        let expr = parse("$name").unwrap();
        assert_eq!(
            expr,
            Expr::FieldRef {
                name: Some("name".into()),
                path: vec![]
            }
        );
    }

    #[test]
    fn test_parse_bare_dollar() {
        let expr = parse("$").unwrap();
        assert_eq!(
            expr,
            Expr::FieldRef {
                name: None,
                path: vec![]
            }
        );
    }

    #[test]
    fn test_parse_field_with_path() {
        let expr = parse("$address.city").unwrap();
        assert_eq!(
            expr,
            Expr::FieldRef {
                name: Some("address".into()),
                path: vec![PathSegment::Dot("city".into())]
            }
        );
    }

    /// Spec: fel-grammar.md §7 — precedence-preserving parse tree.
    /// `1 + 2 * 3` must parse as Add(1, Mul(2, 3)), not Mul(Add(1, 2), 3).
    #[test]
    fn test_parse_arithmetic_precedence() {
        let expr = parse("1 + 2 * 3").unwrap();
        match expr {
            Expr::BinaryOp {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                assert_eq!(*left, Expr::Number(Decimal::from(1)));
                match *right {
                    Expr::BinaryOp {
                        op: BinaryOp::Mul,
                        left: rl,
                        right: rr,
                    } => {
                        assert_eq!(*rl, Expr::Number(Decimal::from(2)));
                        assert_eq!(*rr, Expr::Number(Decimal::from(3)));
                    }
                    other => panic!("expected Mul on right, got {other:?}"),
                }
            }
            other => panic!("expected Add at top, got {other:?}"),
        }
    }

    /// Spec: fel-grammar.md §7 — function call with nested field ref + wildcard.
    #[test]
    fn test_parse_function_call() {
        let expr = parse("sum($items[*].qty)").unwrap();
        match expr {
            Expr::FunctionCall { name, args } => {
                assert_eq!(name, "sum");
                assert_eq!(args.len(), 1);
                assert_eq!(
                    args[0],
                    Expr::FieldRef {
                        name: Some("items".into()),
                        path: vec![PathSegment::Wildcard, PathSegment::Dot("qty".into())],
                    }
                );
            }
            other => panic!("expected FunctionCall, got {other:?}"),
        }
    }

    /// Spec: fel-grammar.md §7 — if...then...else keyword form.
    #[test]
    fn test_parse_if_then_else() {
        let expr = parse("if $x > 0 then 'positive' else 'non-positive'").unwrap();
        match expr {
            Expr::IfThenElse {
                condition,
                then_branch,
                else_branch,
            } => {
                // condition: $x > 0
                match *condition {
                    Expr::BinaryOp {
                        op: BinaryOp::Gt,
                        left,
                        right,
                    } => {
                        assert_eq!(
                            *left,
                            Expr::FieldRef {
                                name: Some("x".into()),
                                path: vec![]
                            }
                        );
                        assert_eq!(*right, Expr::Number(Decimal::from(0)));
                    }
                    other => panic!("expected Gt condition, got {other:?}"),
                }
                assert_eq!(*then_branch, Expr::String("positive".into()));
                assert_eq!(*else_branch, Expr::String("non-positive".into()));
            }
            other => panic!("expected IfThenElse, got {other:?}"),
        }
    }

    /// Spec: fel-grammar.md §7 — if(...) function call form.
    #[test]
    fn test_parse_if_function() {
        let expr = parse("if($x > 0, 'yes', 'no')").unwrap();
        match expr {
            Expr::FunctionCall { name, args } => {
                assert_eq!(name, "if");
                assert_eq!(args.len(), 3);
                match &args[0] {
                    Expr::BinaryOp {
                        op: BinaryOp::Gt, ..
                    } => {}
                    other => panic!("expected Gt as first arg, got {other:?}"),
                }
                assert_eq!(args[1], Expr::String("yes".into()));
                assert_eq!(args[2], Expr::String("no".into()));
            }
            other => panic!("expected FunctionCall, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_if_function_inside_keyword_condition() {
        let expr = parse("if if(null, null, null) then 1 else 2").unwrap();
        match expr {
            Expr::IfThenElse { condition, .. } => match *condition {
                Expr::FunctionCall { name, args } => {
                    assert_eq!(name, "if");
                    assert_eq!(args.as_slice(), &[Expr::Null, Expr::Null, Expr::Null]);
                }
                other => panic!("expected if() FunctionCall condition, got {other:?}"),
            },
            other => panic!("expected IfThenElse, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_nested_if_function_does_not_see_outer_then() {
        let expr = parse("if [if(null, null, null)] then typeOf(1) else floor(2)[2]").unwrap();
        match expr {
            Expr::IfThenElse { condition, .. } => match *condition {
                Expr::Array(elements) => match elements.as_slice() {
                    [Expr::FunctionCall { name, args }] => {
                        assert_eq!(name, "if");
                        assert_eq!(args.as_slice(), &[Expr::Null, Expr::Null, Expr::Null]);
                    }
                    other => panic!("expected array containing if() call, got {other:?}"),
                },
                other => panic!("expected array condition, got {other:?}"),
            },
            other => panic!("expected IfThenElse, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_parenthesized_keyword_if_condition() {
        let expr = parse("if ($x > 0) and true then 'yes' else 'no'").unwrap();
        match expr {
            Expr::IfThenElse { condition, .. } => match *condition {
                Expr::BinaryOp {
                    op: BinaryOp::And, ..
                } => {}
                other => panic!("expected And condition, got {other:?}"),
            },
            other => panic!("expected IfThenElse, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_membership_inside_let_value_containers() {
        let expr = parse(
            "let aa = {'object': null in [], 'array': [null in []], 'call': startsWith(null in [], true)} in aa",
        )
        .unwrap();
        match expr {
            Expr::LetBinding { value, .. } => match *value {
                Expr::Object(entries) => {
                    let [object_entry, array_entry, call_entry] = entries.as_slice() else {
                        panic!("expected three object entries, got {entries:?}");
                    };
                    assert!(matches!(object_entry.1, Expr::Membership { .. }));
                    match &array_entry.1 {
                        Expr::Array(elements) => {
                            assert!(matches!(elements.as_slice(), [Expr::Membership { .. }]));
                        }
                        other => panic!("expected array entry, got {other:?}"),
                    }
                    match &call_entry.1 {
                        Expr::FunctionCall { args, .. } => {
                            assert!(matches!(
                                args.as_slice(),
                                [Expr::Membership { .. }, Expr::Boolean(true)]
                            ));
                        }
                        other => panic!("expected function call entry, got {other:?}"),
                    }
                }
                other => panic!("expected object let value, got {other:?}"),
            },
            other => panic!("expected LetBinding, got {other:?}"),
        }
    }

    /// Spec: fel-grammar.md §7 — ternary `? :` with condition, branches.
    #[test]
    fn test_parse_ternary() {
        let expr = parse("$x > 0 ? 'yes' : 'no'").unwrap();
        match expr {
            Expr::Ternary {
                condition,
                then_branch,
                else_branch,
            } => {
                match *condition {
                    Expr::BinaryOp {
                        op: BinaryOp::Gt,
                        left,
                        right,
                    } => {
                        assert_eq!(
                            *left,
                            Expr::FieldRef {
                                name: Some("x".into()),
                                path: vec![]
                            }
                        );
                        assert_eq!(*right, Expr::Number(Decimal::from(0)));
                    }
                    other => panic!("expected Gt condition, got {other:?}"),
                }
                assert_eq!(*then_branch, Expr::String("yes".into()));
                assert_eq!(*else_branch, Expr::String("no".into()));
            }
            other => panic!("expected Ternary, got {other:?}"),
        }
    }

    /// Spec: fel-grammar.md §7 — let binding structure.
    #[test]
    fn test_parse_let_binding() {
        let expr = parse("let x = 5 in x + 1").unwrap();
        match expr {
            Expr::LetBinding { name, value, body } => {
                assert_eq!(name, "x");
                assert_eq!(*value, Expr::Number(Decimal::from(5)));
                match *body {
                    Expr::BinaryOp {
                        op: BinaryOp::Add,
                        left,
                        right,
                    } => {
                        assert_eq!(
                            *left,
                            Expr::VarRef {
                                name: "x".into(),
                                path: vec![]
                            }
                        );
                        assert_eq!(*right, Expr::Number(Decimal::from(1)));
                    }
                    other => panic!("expected Add in body, got {other:?}"),
                }
            }
            other => panic!("expected LetBinding, got {other:?}"),
        }
    }

    /// Spec: fel-grammar.md §7 — `in` membership with array literal.
    #[test]
    fn test_parse_membership() {
        let expr = parse("$status in ['active', 'pending']").unwrap();
        match expr {
            Expr::Membership {
                value,
                container,
                negated,
            } => {
                assert!(!negated);
                assert_eq!(
                    *value,
                    Expr::FieldRef {
                        name: Some("status".into()),
                        path: vec![]
                    }
                );
                match *container {
                    Expr::Array(ref elems) => {
                        assert_eq!(elems.len(), 2);
                        assert_eq!(elems[0], Expr::String("active".into()));
                        assert_eq!(elems[1], Expr::String("pending".into()));
                    }
                    other => panic!("expected Array container, got {other:?}"),
                }
            }
            other => panic!("expected Membership, got {other:?}"),
        }
    }

    /// Spec: fel-grammar.md §7 — `not in` membership.
    #[test]
    fn test_parse_not_in() {
        let expr = parse("$status not in ['deleted']").unwrap();
        match expr {
            Expr::Membership {
                value,
                container,
                negated,
            } => {
                assert!(negated);
                assert_eq!(
                    *value,
                    Expr::FieldRef {
                        name: Some("status".into()),
                        path: vec![]
                    }
                );
                match *container {
                    Expr::Array(ref elems) => {
                        assert_eq!(elems.len(), 1);
                        assert_eq!(elems[0], Expr::String("deleted".into()));
                    }
                    other => panic!("expected Array container, got {other:?}"),
                }
            }
            other => panic!("expected Membership, got {other:?}"),
        }
    }

    /// Spec: fel-grammar.md §7 — null coalesce `??` structure.
    #[test]
    fn test_parse_null_coalesce() {
        let expr = parse("$x ?? 0").unwrap();
        match expr {
            Expr::NullCoalesce { left, right } => {
                assert_eq!(
                    *left,
                    Expr::FieldRef {
                        name: Some("x".into()),
                        path: vec![]
                    }
                );
                assert_eq!(*right, Expr::Number(Decimal::from(0)));
            }
            other => panic!("expected NullCoalesce, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_context_ref() {
        let expr = parse("@index").unwrap();
        assert_eq!(
            expr,
            Expr::ContextRef {
                name: "index".into(),
                arg: None,
                tail: vec![]
            }
        );
    }

    #[test]
    fn test_parse_context_ref_with_arg() {
        let expr = parse("@instance('priorYear').total").unwrap();
        assert_eq!(
            expr,
            Expr::ContextRef {
                name: "instance".into(),
                arg: Some("priorYear".into()),
                tail: vec!["total".into()]
            }
        );
    }

    #[test]
    fn test_parse_date_literal() {
        let expr = parse("@2024-01-15").unwrap();
        assert_eq!(expr, Expr::DateLiteral("@2024-01-15".into()));
    }

    #[test]
    fn test_parse_object_literal() {
        let expr = parse("{name: 'Alice', age: 30}").unwrap();
        assert!(matches!(expr, Expr::Object(_)));
    }

    #[test]
    fn test_parse_array_literal() {
        let expr = parse("[1, 2, 3]").unwrap();
        assert!(matches!(expr, Expr::Array(_)));
    }

    #[test]
    fn test_parse_wildcard() {
        let expr = parse("$items[*].name").unwrap();
        assert_eq!(
            expr,
            Expr::FieldRef {
                name: Some("items".into()),
                path: vec![PathSegment::Wildcard, PathSegment::Dot("name".into())]
            }
        );
    }

    #[test]
    fn parse_bracket_rejects_fractional_index() {
        let err = parse("$rows[1.5].id").unwrap_err();
        assert!(
            err.to_string().contains("non-negative integer index"),
            "got: {err}"
        );
    }

    #[test]
    fn parse_bracket_error_messages_differ_by_context() {
        let postfix = parse("items[foo]").unwrap_err().to_string();
        let field_ref = parse("$rows[foo]").unwrap_err().to_string();
        assert!(postfix.contains("brackets"), "postfix: {postfix}");
        assert!(
            field_ref.contains("field ref") && field_ref.contains("brackets"),
            "field ref: {field_ref}"
        );
    }

    #[test]
    fn test_parse_bracket_index_field_ref_and_postfix() {
        assert_eq!(
            parse("$rows[2].id").unwrap(),
            Expr::FieldRef {
                name: Some("rows".into()),
                path: vec![PathSegment::Index(2), PathSegment::Dot("id".into())]
            }
        );
        let expr = parse("items[1]").unwrap();
        match expr {
            Expr::PostfixAccess { expr, path } => {
                assert_eq!(
                    *expr,
                    Expr::VarRef {
                        name: "items".into(),
                        path: vec![]
                    }
                );
                assert_eq!(path, vec![PathSegment::Index(1)]);
            }
            other => panic!("expected PostfixAccess, got {other:?}"),
        }
    }

    #[test]
    fn bare_identifier_postfix_is_postfix_access() {
        let expr = parse("x.a.b[0]").unwrap();
        assert!(matches!(expr, Expr::PostfixAccess { .. }));
    }

    #[test]
    fn parenthesized_expression_postfix_is_nested_postfix() {
        let expr = parse("(1 + 2).a.b").unwrap();
        match expr {
            Expr::PostfixAccess { expr, path } => {
                assert!(matches!(*expr, Expr::PostfixAccess { .. }));
                assert_eq!(path, vec![PathSegment::Dot("b".into())]);
            }
            other => panic!("expected PostfixAccess, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_unary_not() {
        let expr = parse("not true").unwrap();
        assert!(matches!(
            expr,
            Expr::UnaryOp {
                op: UnaryOp::Not,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_complex_nested() {
        parse("if $items[*].qty > 0 then sum($items[*].qty) * $rate else 0").unwrap();
    }

    #[test]
    fn test_parse_let_in_does_not_trigger_membership() {
        // `in` in `let x = $a in x + 1` should be the let-body separator,
        // not the membership operator
        let expr = parse("let x = $a in x + 1").unwrap();
        assert!(matches!(expr, Expr::LetBinding { .. }));
    }

    #[test]
    fn test_parse_let_with_parenthesized_in() {
        // Parens reset the no_in suppression, so the inner `in` is membership
        let expr = parse("let x = (1 in [1, 2, 3]) in x").unwrap();
        match expr {
            Expr::LetBinding { value, body, .. } => {
                assert!(matches!(*value, Expr::Membership { negated: false, .. }));
                assert!(matches!(*body, Expr::VarRef { .. }));
            }
            other => panic!("expected LetBinding, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_let_nested_parens_restore_suppression() {
        // After the closing paren, `in` should still be the let-body separator
        let expr = parse("let y = ($a + 1) in y * 2").unwrap();
        match expr {
            Expr::LetBinding { name, .. } => assert_eq!(name, "y"),
            other => panic!("expected LetBinding, got {other:?}"),
        }
    }
}
