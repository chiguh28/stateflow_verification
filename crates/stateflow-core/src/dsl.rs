use crate::{CompareOperator, GuardExpr, InputId, MachineId, OutputId, TypedValue, ValueReference};
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message} at character {position}")]
pub struct GuardParseError {
    pub position: usize,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
enum TokenKind {
    Identifier(String),
    Number(f64),
    String(String),
    LeftParen,
    RightParen,
    Dot,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    End,
}

#[derive(Clone, Debug, PartialEq)]
struct Token {
    kind: TokenKind,
    position: usize,
}

pub fn parse_guard(source: &str) -> Result<GuardExpr, GuardParseError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser { tokens, cursor: 0 };
    let expression = parser.parse_or()?;
    if !matches!(parser.current().kind, TokenKind::End) {
        return Err(parser.error("unexpected token after guard expression"));
    }
    Ok(expression)
}

fn tokenize(source: &str) -> Result<Vec<Token>, GuardParseError> {
    let characters: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while cursor < characters.len() {
        let character = characters[cursor];
        if character.is_whitespace() {
            cursor += 1;
            continue;
        }

        let start = cursor;
        let kind = match character {
            '(' => {
                cursor += 1;
                TokenKind::LeftParen
            }
            ')' => {
                cursor += 1;
                TokenKind::RightParen
            }
            '.' => {
                cursor += 1;
                TokenKind::Dot
            }
            '=' if characters.get(cursor + 1) == Some(&'=') => {
                cursor += 2;
                TokenKind::Equal
            }
            '!' if characters.get(cursor + 1) == Some(&'=') => {
                cursor += 2;
                TokenKind::NotEqual
            }
            '<' if characters.get(cursor + 1) == Some(&'=') => {
                cursor += 2;
                TokenKind::LessOrEqual
            }
            '>' if characters.get(cursor + 1) == Some(&'=') => {
                cursor += 2;
                TokenKind::GreaterOrEqual
            }
            '<' => {
                cursor += 1;
                TokenKind::Less
            }
            '>' => {
                cursor += 1;
                TokenKind::Greater
            }
            '"' | '\'' => {
                let delimiter = character;
                cursor += 1;
                let mut value = String::new();
                let mut closed = false;
                while cursor < characters.len() {
                    if characters[cursor] == delimiter {
                        cursor += 1;
                        closed = true;
                        break;
                    }
                    if characters[cursor] == '\\' {
                        cursor += 1;
                        if cursor >= characters.len() {
                            break;
                        }
                    }
                    value.push(characters[cursor]);
                    cursor += 1;
                }
                if !closed {
                    return Err(GuardParseError {
                        position: start,
                        message: "unterminated string literal".to_owned(),
                    });
                }
                TokenKind::String(value)
            }
            '-' | '0'..='9' => {
                cursor += 1;
                while cursor < characters.len()
                    && (characters[cursor].is_ascii_digit() || characters[cursor] == '.')
                {
                    cursor += 1;
                }
                let text: String = characters[start..cursor].iter().collect();
                let value = text.parse::<f64>().map_err(|_| GuardParseError {
                    position: start,
                    message: format!("invalid number `{text}`"),
                })?;
                if !value.is_finite() {
                    return Err(GuardParseError {
                        position: start,
                        message: "number must be finite".to_owned(),
                    });
                }
                TokenKind::Number(value)
            }
            value if value.is_ascii_alphabetic() || value == '_' => {
                cursor += 1;
                while cursor < characters.len()
                    && (characters[cursor].is_ascii_alphanumeric() || characters[cursor] == '_')
                {
                    cursor += 1;
                }
                TokenKind::Identifier(characters[start..cursor].iter().collect())
            }
            _ => {
                return Err(GuardParseError {
                    position: start,
                    message: format!("unsupported character `{character}`"),
                });
            }
        };
        tokens.push(Token {
            kind,
            position: start,
        });
    }

    tokens.push(Token {
        kind: TokenKind::End,
        position: characters.len(),
    });
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if !matches!(token.kind, TokenKind::End) {
            self.cursor += 1;
        }
        token
    }

    fn error(&self, message: &str) -> GuardParseError {
        GuardParseError {
            position: self.current().position,
            message: message.to_owned(),
        }
    }

    fn parse_or(&mut self) -> Result<GuardExpr, GuardParseError> {
        let mut values = vec![self.parse_and()?];
        while self.is_keyword("OR") {
            self.advance();
            values.push(self.parse_and()?);
        }
        Ok(if values.len() == 1 {
            values.remove(0)
        } else {
            GuardExpr::Or(values)
        })
    }

    fn parse_and(&mut self) -> Result<GuardExpr, GuardParseError> {
        let mut values = vec![self.parse_not()?];
        while self.is_keyword("AND") {
            self.advance();
            values.push(self.parse_not()?);
        }
        Ok(if values.len() == 1 {
            values.remove(0)
        } else {
            GuardExpr::And(values)
        })
    }

    fn parse_not(&mut self) -> Result<GuardExpr, GuardParseError> {
        if self.is_keyword("NOT") {
            self.advance();
            return Ok(GuardExpr::Not(Box::new(self.parse_not()?)));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<GuardExpr, GuardParseError> {
        let left = self.parse_primary()?;
        let operator = match self.current().kind {
            TokenKind::Equal => Some(CompareOperator::Equal),
            TokenKind::NotEqual => Some(CompareOperator::NotEqual),
            TokenKind::Less => Some(CompareOperator::Less),
            TokenKind::LessOrEqual => Some(CompareOperator::LessOrEqual),
            TokenKind::Greater => Some(CompareOperator::Greater),
            TokenKind::GreaterOrEqual => Some(CompareOperator::GreaterOrEqual),
            _ => None,
        };

        let Some(operator) = operator else {
            return Ok(left);
        };
        self.advance();
        let right = self.parse_primary()?;
        if matches!(
            self.current().kind,
            TokenKind::Equal
                | TokenKind::NotEqual
                | TokenKind::Less
                | TokenKind::LessOrEqual
                | TokenKind::Greater
                | TokenKind::GreaterOrEqual
        ) {
            return Err(self.error("comparisons cannot be chained"));
        }
        Ok(GuardExpr::Compare {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    fn parse_primary(&mut self) -> Result<GuardExpr, GuardParseError> {
        if matches!(self.current().kind, TokenKind::LeftParen) {
            self.advance();
            let expression = self.parse_or()?;
            if !matches!(self.current().kind, TokenKind::RightParen) {
                return Err(self.error("expected closing parenthesis"));
            }
            self.advance();
            return Ok(expression);
        }

        match self.advance() {
            Token {
                kind: TokenKind::Number(value),
                ..
            } => Ok(GuardExpr::Literal(TypedValue::Number(value))),
            Token {
                kind: TokenKind::String(value),
                ..
            } => Ok(GuardExpr::Literal(TypedValue::String(value))),
            Token {
                kind: TokenKind::Identifier(name),
                position,
            } => self.parse_identifier(name, position),
            token if matches!(token.kind, TokenKind::End) => Err(GuardParseError {
                position: token.position,
                message: "expected a value or reference".to_owned(),
            }),
            token => Err(GuardParseError {
                position: token.position,
                message: "expected a value or reference".to_owned(),
            }),
        }
    }

    fn parse_identifier(
        &mut self,
        name: String,
        position: usize,
    ) -> Result<GuardExpr, GuardParseError> {
        if matches!(self.current().kind, TokenKind::LeftParen) {
            self.advance();
            let argument = self.parse_path()?;
            if !matches!(self.current().kind, TokenKind::RightParen) {
                return Err(self.error("expected closing parenthesis after function argument"));
            }
            self.advance();
            let input_id = match argument.strip_prefix("INPUT.") {
                Some(value) => value,
                None if !argument.contains('.') => argument.as_str(),
                None => {
                    return Err(GuardParseError {
                        position,
                        message: "RISE/FALL expects an input identifier".to_owned(),
                    });
                }
            };
            return match name.as_str() {
                "RISE" => Ok(GuardExpr::Rise(InputId(input_id.to_owned()))),
                "FALL" => Ok(GuardExpr::Fall(InputId(input_id.to_owned()))),
                _ => Err(GuardParseError {
                    position,
                    message: format!("unknown function `{name}`"),
                }),
            };
        }

        let mut path = name;
        while matches!(self.current().kind, TokenKind::Dot) {
            self.advance();
            match self.advance() {
                Token {
                    kind: TokenKind::Identifier(part),
                    ..
                } => {
                    path.push('.');
                    path.push_str(&part);
                }
                token => {
                    return Err(GuardParseError {
                        position: token.position,
                        message: "expected an identifier after `.`".to_owned(),
                    });
                }
            }
        }

        match path.as_str() {
            "TRUE" => Ok(GuardExpr::Literal(TypedValue::Boolean(true))),
            "FALSE" => Ok(GuardExpr::Literal(TypedValue::Boolean(false))),
            "UNKNOWN" => Ok(GuardExpr::Literal(TypedValue::Unknown)),
            _ => {
                if let Some(reference) = parse_reference(&path) {
                    Ok(GuardExpr::Reference(reference))
                } else if path.contains('.') {
                    Err(GuardParseError {
                        position,
                        message: format!("invalid reference `{path}`"),
                    })
                } else {
                    Ok(GuardExpr::Literal(TypedValue::Enum(path)))
                }
            }
        }
    }

    fn parse_path(&mut self) -> Result<String, GuardParseError> {
        let first = match self.advance() {
            Token {
                kind: TokenKind::Identifier(value),
                ..
            } => value,
            token => {
                return Err(GuardParseError {
                    position: token.position,
                    message: "expected input identifier".to_owned(),
                });
            }
        };
        let mut path = first;
        while matches!(self.current().kind, TokenKind::Dot) {
            self.advance();
            match self.advance() {
                Token {
                    kind: TokenKind::Identifier(part),
                    ..
                } => {
                    path.push('.');
                    path.push_str(&part);
                }
                token => {
                    return Err(GuardParseError {
                        position: token.position,
                        message: "expected identifier after `.`".to_owned(),
                    });
                }
            }
        }
        Ok(path)
    }

    fn is_keyword(&self, keyword: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Identifier(value) if value == keyword)
    }
}

fn parse_reference(path: &str) -> Option<ValueReference> {
    let parts: Vec<&str> = path.split('.').collect();
    match parts.as_slice() {
        ["INPUT", id] => Some(ValueReference::Input(InputId((*id).to_owned()))),
        ["STATE", machine] => Some(ValueReference::State(MachineId((*machine).to_owned()))),
        ["BEFORE", "STATE", machine] => Some(ValueReference::BeforeState(MachineId(
            (*machine).to_owned(),
        ))),
        ["PREV", "STATE", machine] => Some(ValueReference::PreviousState(MachineId(
            (*machine).to_owned(),
        ))),
        ["PREV", "OUTPUT", output] => Some(ValueReference::PreviousOutput(OutputId(
            (*output).to_owned(),
        ))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_guard;
    use crate::{CompareOperator, GuardExpr, InputId, MachineId, TypedValue, ValueReference};

    #[test]
    fn parser_respects_not_and_boolean_precedence() {
        let expression = parse_guard("NOT INPUT.SAFETY == ON AND STATE.AC == AUTO OR FALSE")
            .expect("guard should parse");

        assert!(matches!(expression, GuardExpr::Or(_)));
        let GuardExpr::Or(parts) = expression else {
            unreachable!();
        };
        assert_eq!(parts.len(), 2);
        assert!(matches!(parts[0], GuardExpr::And(_)));
        assert_eq!(parts[1], GuardExpr::Literal(TypedValue::Boolean(false)));
    }

    #[test]
    fn parser_keeps_history_reference_kinds_distinct() {
        assert_eq!(
            parse_guard("PREV.STATE.AC == MANUAL").expect("PREV guard"),
            GuardExpr::Compare {
                operator: CompareOperator::Equal,
                left: Box::new(GuardExpr::Reference(ValueReference::PreviousState(
                    MachineId("AC".to_owned())
                ))),
                right: Box::new(GuardExpr::Literal(TypedValue::Enum("MANUAL".to_owned())))
            }
        );
        assert!(parse_guard("BEFORE.STATE.AC == MANUAL").is_ok());
    }

    #[test]
    fn rise_requires_a_single_input_name_and_comparisons_do_not_chain() {
        assert_eq!(
            parse_guard("RISE(INPUT.AUTO_SW)").expect("RISE guard"),
            GuardExpr::Rise(InputId("AUTO_SW".to_owned()))
        );
        assert!(parse_guard("INPUT.A < 3 < 5").is_err());
        assert!(parse_guard("FALL(STATE.AC)").is_err());
    }
}
