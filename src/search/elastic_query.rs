use std::collections::HashMap;
use std::collections::HashSet;
use std::iter::Peekable;
use std::str::Chars;

/// The AST representing a parsed query.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A search term, which can represent multiple keywords.
    /// `keywords` => a list of keywords (e.g. ["white", "list"] for "whitelisting")
    /// `field` => optional field specifier (e.g. Some("comment") for "comment:whitelisting")
    /// `required` => a leading `+`
    /// `excluded` => a leading `-`
    /// `exact` => quoted string for exact matching without stemming
    Term {
        keywords: Vec<String>,
        field: Option<String>,
        required: bool,
        excluded: bool,
        exact: bool,
    },

    /// Logical AND of two sub-expressions.
    And(Box<Expr>, Box<Expr>),

    /// Logical OR of two sub-expressions.
    Or(Box<Expr>, Box<Expr>),
}

impl Expr {
    /// Extract required and optional terms from the AST, excluding negative terms
    #[cfg(test)]
    pub fn extract_terms(&self) -> (Vec<String>, Vec<String>) {
        let mut required = Vec::new();
        let mut optional = Vec::new();
        self.collect_terms(&mut required, &mut optional);
        (required, optional)
    }

    #[cfg(test)]
    fn collect_terms(&self, required: &mut Vec<String>, optional: &mut Vec<String>) {
        match self {
            Expr::Term {
                keywords,
                required: is_required,
                excluded,
                ..
            } => {
                if !excluded {
                    for keyword in keywords {
                        if *is_required {
                            required.push(keyword.clone());
                        } else {
                            optional.push(keyword.clone());
                        }
                    }
                }
            }
            Expr::And(left, right) => {
                left.collect_terms(required, optional);
                right.collect_terms(required, optional);
            }
            Expr::Or(left, right) => {
                left.collect_terms(required, optional);
                right.collect_terms(required, optional);
            }
        }
    }

    /// Evaluate whether a set of matched term indices satisfies this logical expression.
    ///
    /// This is a simplified evaluation function that handles the following cases:
    /// - Term evaluation (checking if all keywords in a term are present)
    /// - AND expressions (both sides must evaluate to true)
    /// - OR expressions (at least one side must evaluate to true)
    /// - Required terms (must be present)
    /// - Excluded terms (must not be present)
    /// - Optional terms (contribute to matching if present)
    ///
    /// Parameters:
    /// - `matched_terms`: the set of term indices that are matched in the block/file.
    /// - `term_indices`: a mapping from a *string term* (e.g., "foo") to a unique index (e.g., 0).
    ///
    /// Returns `true` if the logical expression is satisfied, `false` otherwise.
    pub fn evaluate(
        &self,
        matched_terms: &HashSet<usize>,
        term_indices: &HashMap<String, usize>,
    ) -> bool {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

        if debug_mode {
            println!("DEBUG: Evaluating expression: {:?}", self);
            println!("DEBUG: Matched terms: {:?}", matched_terms);
            println!("DEBUG: Term indices: {:?}", term_indices);
        }

        match self {
            Expr::Term {
                keywords,
                field: _, // Field is handled during term matching, not evaluation
                required,
                excluded,
                exact,
            } => {
                // Edge case: If `keywords` is empty
                if keywords.is_empty() {
                    return !excluded; // Empty excluded term is true, otherwise false
                }

                // Check if *all* of these keywords are present in the matched terms
                let all_present = keywords.iter().all(|kw| {
                    // First try the exact keyword
                    if let Some(&idx) = term_indices.get(kw) {
                        let result = matched_terms.contains(&idx);
                        if debug_mode {
                            println!("DEBUG: Keyword '{}' (idx: {}) is present: {}", kw, idx, result);
                        }
                        if result {
                            return true;
                        }
                    }

                    // For excluded or exact terms, we should be more strict and not use stemming
                    // This prevents false positives when checking for excluded terms
                    if !*excluded && !*exact {
                        // Try to find a stemmed version of the keyword
                        // This handles cases where we're looking for a term but only have its stemmed version in the matched terms
                        for (term, &idx) in term_indices {
                            // Check if this term could be a stemmed version of our keyword
                            // Simple heuristic: the term is shorter and the keyword starts with it
                            if term.len() < kw.len() && kw.starts_with(term) {
                                let result = matched_terms.contains(&idx);
                                if result {
                                    if debug_mode {
                                        println!("DEBUG: Possible stemmed keyword '{}' for '{}' (idx: {}) is present: {}",
                                                term, kw, idx, result);
                                    }
                                    return true;
                                }
                            }
                        }
                    }

                    if debug_mode {
                        println!("DEBUG: Keyword '{}' not found in term_indices", kw);
                    }
                    false
                });

                if *excluded {
                    // Excluded => the block must NOT contain all of them
                    if debug_mode {
                        println!(
                            "DEBUG: Excluded term, all_present={}, returning={}",
                            all_present, !all_present
                        );
                    }
                    !all_present
                } else if *required {
                    // Required => the block *must* contain them all
                    if debug_mode {
                        println!(
                            "DEBUG: Required term, all_present={}, returning={}",
                            all_present, all_present
                        );
                    }
                    all_present
                } else {
                    // Optional => if they're all present, this term is "true"; else false
                    if debug_mode {
                        println!(
                            "DEBUG: Optional term, all_present={}, returning={}",
                            all_present, all_present
                        );
                    }
                    all_present
                }
            }
            Expr::And(left, right) => {
                let left_result = left.evaluate(matched_terms, term_indices);
                let right_result = right.evaluate(matched_terms, term_indices);
                let result = left_result && right_result;
                if debug_mode {
                    println!(
                        "DEBUG: AND expression: left={}, right={}, result={}",
                        left_result, right_result, result
                    );
                }
                result
            }
            Expr::Or(left, right) => {
                let left_result = left.evaluate(matched_terms, term_indices);
                let right_result = right.evaluate(matched_terms, term_indices);
                let result = left_result || right_result;
                if debug_mode {
                    println!(
                        "DEBUG: OR expression: left={}, right={}, result={}",
                        left_result, right_result, result
                    );
                }
                result
            }
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Term {
                keywords,
                field,
                required,
                excluded,
                exact,
            } => {
                let prefix = if *required {
                    "+"
                } else if *excluded {
                    "-"
                } else {
                    ""
                };

                // Add field prefix if present
                let field_prefix = if let Some(field_name) = field {
                    format!("{}:", field_name)
                } else {
                    String::new()
                };

                // Join multiple keywords with spaces
                let keyword_str = if keywords.len() == 1 {
                    if *exact {
                        format!("\"{}\"", keywords[0])
                    } else {
                        keywords[0].clone()
                    }
                } else {
                    format!("\"{}\"", keywords.join(" "))
                };

                write!(f, "{}{}{}", prefix, field_prefix, keyword_str)
            }
            Expr::And(left, right) => write!(f, "({} AND {})", left, right),
            Expr::Or(left, right) => write!(f, "({} OR {})", left, right),
        }
    }
}

/// Our possible tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Plus,                 // '+'
    Minus,                // '-'
    And,                  // 'AND'
    Or,                   // 'OR'
    LParen,               // '('
    RParen,               // ')'
    Ident(String),        // e.g. 'foo', 'bar123'
    QuotedString(String), // e.g. "hello world"
}

/// A simple error type for parsing/tokenizing.
#[derive(Debug)]
pub enum ParseError {
    #[allow(dead_code)]
    UnexpectedChar(char),
    UnexpectedEndOfInput,
    UnexpectedToken(Token),
    Generic(String),
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedChar(c) => write!(f, "Unexpected character '{}'", c),
            ParseError::UnexpectedEndOfInput => write!(f, "Unexpected end of input"),
            ParseError::UnexpectedToken(t) => write!(f, "Unexpected token '{:?}'", t),
            ParseError::Generic(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for ParseError {}

/// Tokenize input string into a vector of tokens
fn tokenize(input: &str) -> Result<Vec<Token>, ParseError> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    while let Some(&ch) = chars.peek() {
        match ch {
            // Skip whitespace
            c if c.is_whitespace() => {
                chars.next();
            }

            // Single-character tokens
            '+' => {
                tokens.push(Token::Plus);
                chars.next();
            }
            '-' => {
                tokens.push(Token::Minus);
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }

            // Add a new case for quoted strings
            '"' => {
                chars.next(); // consume the opening quote
                let quoted_string = lex_quoted_string(&mut chars)?;
                tokens.push(Token::QuotedString(quoted_string));
            }

            // Possible AND/OR keywords or just an identifier
            _ => {
                // If it starts with a letter, number, underscore, or dot, treat it as an identifier
                if ch.is_alphanumeric() || ch == '_' || ch == '.' {
                    let ident = lex_identifier(&mut chars);
                    let ident_upper = ident.to_ascii_uppercase();
                    if ident_upper == "AND" {
                        tokens.push(Token::And);
                    } else if ident_upper == "OR" {
                        tokens.push(Token::Or);
                    } else {
                        tokens.push(Token::Ident(ident));
                    }
                } else {
                    // Skip unknown characters instead of returning an error
                    if debug_mode {
                        println!("DEBUG: Skipping unknown character: '{}'", ch);
                    }
                    chars.next();
                }
            }
        }
    }

    // If we have no tokens, return an error
    if tokens.is_empty() {
        return Err(ParseError::Generic(
            "No valid tokens found in input".to_string(),
        ));
    }

    Ok(tokens)
}

// New helper function to lex a quoted string
fn lex_quoted_string(chars: &mut Peekable<Chars>) -> Result<String, ParseError> {
    let mut buf = String::new();
    let mut escaped = false;

    while let Some(&ch) = chars.peek() {
        if escaped {
            buf.push(ch);
            escaped = false;
            chars.next();
        } else if ch == '\\' {
            escaped = true;
            chars.next();
        } else if ch == '"' {
            chars.next(); // consume the closing quote
            return Ok(buf);
        } else {
            buf.push(ch);
            chars.next();
        }
    }

    // If we get here, we ran out of characters before finding a closing quote
    Err(ParseError::UnexpectedEndOfInput)
}

fn lex_identifier(chars: &mut Peekable<Chars>) -> String {
    let mut buf = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_alphanumeric() || ch == '_' || ch == '.' {
            buf.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    buf
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    any_term: bool,
}

impl Parser {
    fn new(tokens: Vec<Token>, any_term: bool) -> Self {
        Parser {
            tokens,
            pos: 0,
            any_term,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.peek()?.clone();
        self.pos += 1;
        Some(t)
    }

    fn expect(&mut self, expected: &Token) -> Result<Token, ParseError> {
        match self.peek() {
            Some(t) if t == expected => Ok(self.next().unwrap()),
            Some(t) => Err(ParseError::UnexpectedToken(t.clone())),
            None => Err(ParseError::UnexpectedEndOfInput),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or_expr()
    }

    /// Parses an OR expression, which has the lowest precedence.
    fn parse_or_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and_expr()?;

        while let Some(Token::Or) = self.peek() {
            self.next(); // consume OR
            let right = self.parse_and_expr()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    /// Parses an AND expression, which binds tighter than OR.
    fn parse_and_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_factor()?;

        while let Some(token) = self.peek() {
            match token {
                Token::And => {
                    self.next(); // consume AND
                    let right = self.parse_factor()?;
                    left = Expr::And(Box::new(left), Box::new(right));
                }
                Token::Plus
                | Token::Minus
                | Token::LParen
                | Token::Ident(_)
                | Token::QuotedString(_) => {
                    let right = self.parse_factor()?;
                    // Use OR for implicit combinations (space-separated terms) in any_term mode
                    // Use AND for implicit combinations in all_terms mode
                    if self.any_term {
                        left = Expr::Or(Box::new(left), Box::new(right));
                    } else {
                        left = Expr::And(Box::new(left), Box::new(right));
                    }
                }
                _ => break,
            }
        }

        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(Token::LParen) => {
                self.next();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            _ => self.parse_prefixed_term(),
        }
    }

    fn parse_prefixed_term(&mut self) -> Result<Expr, ParseError> {
        let mut required = false;
        let mut excluded = false;

        match self.peek() {
            Some(Token::Plus) => {
                required = true;
                self.next();
            }
            Some(Token::Minus) => {
                excluded = true;
                self.next();
            }
            _ => {}
        }

        let primary_expr = self.parse_primary()?;

        if let Expr::Term {
            keywords,
            field,
            exact,
            ..
        } = primary_expr
        {
            Ok(Expr::Term {
                keywords,
                field,
                required,
                excluded,
                exact,
            })
        } else {
            Ok(primary_expr)
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(Token::Ident(_)) => {
                if let Some(Token::Ident(kw)) = self.next() {
                    // Check if this is a field specifier (e.g., "field:term")
                    let mut field = None;
                    let mut keywords = Vec::new();

                    if kw.contains(':') {
                        let parts: Vec<&str> = kw.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            field = Some(parts[0].to_string());
                            // For now, treat the term as a single keyword
                            // In Milestone 2, we'll apply tokenization and stemming here
                            keywords.push(parts[1].to_string());
                        } else {
                            // Invalid format, treat as a regular term
                            keywords.push(kw);
                        }
                    } else {
                        // Regular term, no field specifier
                        keywords.push(kw);
                    }

                    Ok(Expr::Term {
                        keywords,
                        field,
                        required: false,
                        excluded: false,
                        exact: false,
                    })
                } else {
                    unreachable!();
                }
            }
            Some(Token::QuotedString(s)) => {
                let string_value = s.clone();
                self.next();
                Ok(Expr::Term {
                    keywords: vec![string_value],
                    field: None,
                    required: false,
                    excluded: false,
                    exact: true, // Mark as exact match
                })
            }
            Some(Token::LParen) => {
                self.next();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Some(t) => Err(ParseError::UnexpectedToken(t.clone())),
            None => Err(ParseError::UnexpectedEndOfInput),
        }
    }
}

/// Process the AST terms by applying tokenization and stemming
fn process_ast_terms(expr: Expr) -> Expr {
    use crate::search::tokenization::tokenize;

    match expr {
        Expr::Term {
            keywords,
            field,
            required,
            excluded,
            exact,
        } => {
            // For excluded or exact terms, don't apply tokenization
            if excluded || exact {
                return Expr::Term {
                    keywords,
                    field,
                    required,
                    excluded,
                    exact,
                };
            }

            // Apply tokenization to the keywords for non-excluded, non-exact terms
            let processed_keywords = keywords
                .iter()
                .flat_map(|keyword| {
                    // For terms with underscores, we need to tokenize them properly
                    // This will split "keyword_underscore" into ["key", "word", "under", "score"]
                    tokenize(keyword)
                })
                .collect();

            Expr::Term {
                keywords: processed_keywords,
                field,
                required,
                excluded,
                exact,
            }
        }
        Expr::And(left, right) => Expr::And(
            Box::new(process_ast_terms(*left)),
            Box::new(process_ast_terms(*right)),
        ),
        Expr::Or(left, right) => Expr::Or(
            Box::new(process_ast_terms(*left)),
            Box::new(process_ast_terms(*right)),
        ),
    }
}

/// Parse the query string into an AST
pub fn parse_query(input: &str, any_term: bool) -> Result<Expr, ParseError> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Try to tokenize the input
    let tokens_result = tokenize(input);

    if let Err(e) = &tokens_result {
        if debug_mode {
            println!("DEBUG: Tokenization failed: {:?}", e);
        }

        // If tokenization fails, extract any valid identifiers from the input
        // and create a simple Term expression with them
        let cleaned_input = input
            .chars()
            .filter(|&c| c.is_alphanumeric() || c.is_whitespace() || c == '_' || c == '.')
            .collect::<String>();

        if debug_mode {
            println!("DEBUG: Cleaned input: '{}'", cleaned_input);
        }

        if cleaned_input.trim().is_empty() {
            return Err(ParseError::Generic(
                "No valid tokens found in input".to_string(),
            ));
        }

        // Create a simple Term expression with the cleaned input
        let keywords = cleaned_input
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect::<Vec<String>>();

        if debug_mode {
            println!("DEBUG: Created fallback keywords: {:?}", keywords);
        }

        return Ok(Expr::Term {
            keywords,
            field: None,
            required: false,
            excluded: false,
            exact: false,
        });
    }

    let tokens = tokens_result.unwrap();

    // Pass any_term to the parser to control how implicit combinations are handled
    let mut parser = Parser::new(tokens, any_term);

    // Try to parse the tokens into an AST
    let raw_ast_result = parser.parse_expr();

    if let Err(e) = &raw_ast_result {
        if debug_mode {
            println!("DEBUG: AST parsing failed: {:?}", e);
        }

        // If parsing fails, extract any identifiers from the tokens
        // and create a simple Term expression with them
        let idents = parser
            .tokens
            .iter()
            .filter_map(|t| match t {
                Token::Ident(s) => Some(s.clone()),
                _ => None,
            })
            .collect::<Vec<String>>();

        if debug_mode {
            println!("DEBUG: Extracted identifiers from tokens: {:?}", idents);
        }

        if idents.is_empty() {
            return Err(ParseError::Generic(
                "No valid identifiers found in tokens".to_string(),
            ));
        }
        return Ok(Expr::Term {
            keywords: idents,
            field: None,
            required: false,
            excluded: false,
            exact: false,
        });
    }

    let raw_ast = raw_ast_result.unwrap();

    // Check if we consumed all tokens
    if parser.pos < parser.tokens.len() && debug_mode {
        println!("DEBUG: Extra tokens after complete parse, using partial AST");
    }

    // Apply tokenization and stemming to the AST
    let processed_ast = process_ast_terms(raw_ast);

    Ok(processed_ast)
}

/// Backward compatibility wrapper for parse_query
#[allow(dead_code)]
pub fn parse_query_compat(input: &str) -> Result<Expr, ParseError> {
    // Default to any_term = true for backward compatibility
    parse_query(input, true)
}

// For tests only - this allows tests to call parse_query without the any_term parameter
#[cfg(test)]
pub fn parse_query_test(input: &str) -> Result<Expr, ParseError> {
    parse_query(input, true)
}

#[cfg(test)]
mod tests {
    include!("elastic_query_tests.rs");
    include!("elastic_query_evaluate_tests.rs");
    include!("elastic_query_tokenization_tests.rs");
}
