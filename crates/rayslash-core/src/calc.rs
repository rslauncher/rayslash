#[derive(Debug, Clone, PartialEq)]
pub struct Calculation {
    pub expression: String,
    pub result: String,
}

pub fn calculate(query: &str) -> Option<Calculation> {
    let expression = query.trim();
    if expression.is_empty() || !contains_only_math_chars(expression) {
        return None;
    }

    let mut parser = Parser::new(expression);
    let value = parser.parse_expression().ok()?;
    parser.skip_whitespace();

    if parser.has_remaining() || parser.binary_operator_count == 0 || !value.is_finite() {
        return None;
    }

    Some(Calculation {
        expression: expression.to_owned(),
        result: format_result(value),
    })
}

fn contains_only_math_chars(expression: &str) -> bool {
    expression.chars().all(|ch| {
        ch.is_ascii_digit() || matches!(ch, '+' | '-' | '*' | '/' | '(' | ')' | '.' | ' ' | '\t')
    })
}

fn format_result(value: f64) -> String {
    let value = if value == -0.0 { 0.0 } else { value };
    value.to_string()
}

struct Parser<'a> {
    input: &'a str,
    position: usize,
    binary_operator_count: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            position: 0,
            binary_operator_count: 0,
        }
    }

    fn parse_expression(&mut self) -> Result<f64, ()> {
        self.parse_addition()
    }

    fn parse_addition(&mut self) -> Result<f64, ()> {
        let mut value = self.parse_multiplication()?;

        loop {
            self.skip_whitespace();

            if self.consume('+') {
                self.binary_operator_count += 1;
                value += self.parse_multiplication()?;
            } else if self.consume('-') {
                self.binary_operator_count += 1;
                value -= self.parse_multiplication()?;
            } else {
                break;
            }
        }

        Ok(value)
    }

    fn parse_multiplication(&mut self) -> Result<f64, ()> {
        let mut value = self.parse_unary()?;

        loop {
            self.skip_whitespace();

            if self.consume('*') {
                self.binary_operator_count += 1;
                value *= self.parse_unary()?;
            } else if self.consume('/') {
                self.binary_operator_count += 1;
                let rhs = self.parse_unary()?;
                if rhs == 0.0 {
                    return Err(());
                }
                value /= rhs;
            } else {
                break;
            }
        }

        Ok(value)
    }

    fn parse_unary(&mut self) -> Result<f64, ()> {
        self.skip_whitespace();

        if self.consume('+') {
            self.parse_unary()
        } else if self.consume('-') {
            Ok(-self.parse_unary()?)
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<f64, ()> {
        self.skip_whitespace();

        if self.consume('(') {
            let value = self.parse_expression()?;
            self.skip_whitespace();
            if self.consume(')') {
                Ok(value)
            } else {
                Err(())
            }
        } else {
            self.parse_number()
        }
    }

    fn parse_number(&mut self) -> Result<f64, ()> {
        self.skip_whitespace();

        let start = self.position;
        let mut digit_count = 0;
        let mut saw_decimal_point = false;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                digit_count += 1;
                self.advance(ch);
            } else if ch == '.' && !saw_decimal_point {
                saw_decimal_point = true;
                self.advance(ch);
            } else {
                break;
            }
        }

        if digit_count == 0 {
            return Err(());
        }

        self.input[start..self.position]
            .parse::<f64>()
            .map_err(|_| ())
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek()
            && ch.is_ascii_whitespace()
        {
            self.advance(ch);
        }
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.position += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn advance(&mut self, ch: char) {
        self.position += ch.len_utf8();
    }

    fn has_remaining(&self) -> bool {
        self.position < self.input.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(query: &str) -> Option<String> {
        calculate(query).map(|calculation| calculation.result)
    }

    #[test]
    fn detects_simple_math_expressions() {
        assert_eq!(result("1+2"), Some("3".to_owned()));
        assert_eq!(result("10 - 4"), Some("6".to_owned()));
        assert_eq!(result("2 * 3"), Some("6".to_owned()));
        assert_eq!(result("8 / 2"), Some("4".to_owned()));
        assert_eq!(result("0.5 + 1.25"), Some("1.75".to_owned()));
    }

    #[test]
    fn respects_precedence_and_parentheses() {
        assert_eq!(result("2 + 3 * 4"), Some("14".to_owned()));
        assert_eq!(result("(2 + 3) * 4"), Some("20".to_owned()));
        assert_eq!(result("-(2 + 3) * 4"), Some("-20".to_owned()));
    }

    #[test]
    fn rejects_invalid_or_incomplete_expressions() {
        assert_eq!(result(""), None);
        assert_eq!(result("code"), None);
        assert_eq!(result("calculator"), None);
        assert_eq!(result("42"), None);
        assert_eq!(result("1 +"), None);
        assert_eq!(result("(1 + 2"), None);
        assert_eq!(result("1 / 0"), None);
        assert_eq!(result("1..2 + 3"), None);
        assert_eq!(result("2(3 + 4)"), None);
    }
}
