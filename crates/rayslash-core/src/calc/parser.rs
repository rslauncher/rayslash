use super::error::CalcError;

pub(super) struct Parser<'a> {
    input: &'a str,
    position: usize,
    binary_operator_count: usize,
    function_count: usize,
    implicit_multiplication_count: usize,
    superscript_exponent_count: usize,
}

impl<'a> Parser<'a> {
    pub(super) fn new(input: &'a str) -> Self {
        Self {
            input,
            position: 0,
            binary_operator_count: 0,
            function_count: 0,
            implicit_multiplication_count: 0,
            superscript_exponent_count: 0,
        }
    }

    pub(super) fn parse_expression(&mut self) -> Result<f64, CalcError> {
        self.parse_addition()
    }

    fn parse_addition(&mut self) -> Result<f64, CalcError> {
        let mut value = self.parse_multiplication()?;

        loop {
            self.skip_whitespace();

            if self.consume('+') {
                self.binary_operator_count += 1;
                value += self.parse_multiplication().map_err(incomplete_rhs)?;
            } else if self.consume('-') {
                self.binary_operator_count += 1;
                value -= self.parse_multiplication().map_err(incomplete_rhs)?;
            } else {
                break;
            }
        }

        Ok(value)
    }

    fn parse_multiplication(&mut self) -> Result<f64, CalcError> {
        let mut value = self.parse_unary()?;

        loop {
            self.skip_whitespace();

            if self.consume('*') {
                self.binary_operator_count += 1;
                value *= self.parse_unary().map_err(incomplete_rhs)?;
            } else if self.consume('/') {
                self.binary_operator_count += 1;
                let rhs = self.parse_unary().map_err(incomplete_rhs)?;
                if rhs == 0.0 {
                    return Err(CalcError::DivisionByZero);
                }
                value /= rhs;
            } else if self.starts_implicit_multiplication() {
                self.implicit_multiplication_count += 1;
                value *= self.parse_unary()?;
            } else {
                break;
            }
        }

        Ok(value)
    }

    fn parse_unary(&mut self) -> Result<f64, CalcError> {
        self.skip_whitespace();

        if self.consume('+') {
            self.parse_unary().map_err(incomplete_rhs)
        } else if self.consume('-') {
            Ok(-self.parse_unary().map_err(incomplete_rhs)?)
        } else {
            self.parse_power()
        }
    }

    fn parse_power(&mut self) -> Result<f64, CalcError> {
        let mut base = self.parse_primary()?;
        self.skip_whitespace();

        if self.consume_power_operator() {
            self.binary_operator_count += 1;
            let exponent = self.parse_unary().map_err(incomplete_rhs)?;
            base = base.powf(exponent);
            if !base.is_finite() {
                return Err(CalcError::NonFiniteResult);
            }
        } else if let Some(exponent) = self.consume_superscript_exponent() {
            self.superscript_exponent_count += 1;
            base = base.powf(exponent);
            if !base.is_finite() {
                return Err(CalcError::NonFiniteResult);
            }
        }

        Ok(base)
    }

    fn parse_primary(&mut self) -> Result<f64, CalcError> {
        self.skip_whitespace();

        if self.consume('(') {
            let value = self.parse_expression()?;
            self.skip_whitespace();
            if self.consume(')') {
                Ok(value)
            } else {
                Err(CalcError::MissingClosingParenthesis)
            }
        } else if self.peek().is_some_and(|ch| ch.is_ascii_alphabetic()) {
            self.parse_identifier()
        } else {
            self.parse_number()
        }
    }

    fn parse_identifier(&mut self) -> Result<f64, CalcError> {
        let start = self.position;

        while let Some(ch) = self.peek()
            && ch.is_ascii_alphanumeric()
        {
            self.advance(ch);
        }

        let identifier = self.input[start..self.position].to_ascii_lowercase();
        self.skip_whitespace();

        if self.consume('(') {
            let value = self.parse_expression()?;
            self.skip_whitespace();
            if !self.consume(')') {
                return Err(CalcError::MissingClosingParenthesis);
            }

            self.function_count += 1;
            return apply_function(&identifier, value);
        }

        match identifier.as_str() {
            "pi" => Ok(std::f64::consts::PI),
            "e" => Ok(std::f64::consts::E),
            _ => Err(CalcError::UnknownIdentifier),
        }
    }

    fn parse_number(&mut self) -> Result<f64, CalcError> {
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
            return Err(CalcError::ExpectedNumber);
        }

        self.input[start..self.position]
            .parse::<f64>()
            .map_err(|_| CalcError::ExpectedNumber)
    }

    pub(super) fn skip_whitespace(&mut self) {
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

    fn consume_power_operator(&mut self) -> bool {
        if self.input[self.position..].starts_with("**") {
            self.position += 2;
            true
        } else {
            self.consume('^')
        }
    }

    fn consume_superscript_exponent(&mut self) -> Option<f64> {
        let start = self.position;
        let mut sign = 1.0;

        if self.consume('⁻') {
            sign = -1.0;
        } else {
            self.consume('⁺');
        }

        let mut value: i64 = 0;
        let mut digit_count = 0;

        while let Some(ch) = self.peek() {
            let Some(digit) = superscript_digit_value(ch) else {
                break;
            };
            digit_count += 1;
            value = value.saturating_mul(10).saturating_add(i64::from(digit));
            self.advance(ch);
        }

        if digit_count == 0 {
            self.position = start;
            None
        } else {
            Some(sign * value as f64)
        }
    }

    fn starts_implicit_multiplication(&self) -> bool {
        self.peek()
            .is_some_and(|ch| ch == '(' || ch.is_ascii_alphabetic())
    }

    pub(super) fn peek(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn advance(&mut self, ch: char) {
        self.position += ch.len_utf8();
    }

    pub(super) fn saw_calculation_signal(&self) -> bool {
        self.binary_operator_count > 0
            || self.function_count > 0
            || self.implicit_multiplication_count > 0
            || self.superscript_exponent_count > 0
    }
}

pub(super) fn has_invalid_operator_sequence(expression: &str) -> bool {
    expression.contains("++")
}

pub(super) fn incomplete_rhs(error: CalcError) -> CalcError {
    match error {
        CalcError::ExpectedNumber => CalcError::IncompleteExpression,
        error => error,
    }
}

pub(super) fn apply_function(name: &str, value: f64) -> Result<f64, CalcError> {
    let result = match name {
        "abs" => value.abs(),
        "acos" => value.acos(),
        "asin" => value.asin(),
        "atan" => value.atan(),
        "ceil" => value.ceil(),
        "cos" => value.cos(),
        "exp" => value.exp(),
        "floor" => value.floor(),
        "ln" => value.ln(),
        "log" | "log10" => value.log10(),
        "round" => value.round(),
        "sin" => value.sin(),
        "sqrt" => value.sqrt(),
        "tan" => value.tan(),
        _ => return Err(CalcError::UnknownFunction),
    };

    if result.is_finite() {
        Ok(result)
    } else {
        Err(CalcError::DomainError)
    }
}

pub(super) fn superscript_digit_value(ch: char) -> Option<u8> {
    match ch {
        '⁰' => Some(0),
        '¹' => Some(1),
        '²' => Some(2),
        '³' => Some(3),
        '⁴' => Some(4),
        '⁵' => Some(5),
        '⁶' => Some(6),
        '⁷' => Some(7),
        '⁸' => Some(8),
        '⁹' => Some(9),
        _ => None,
    }
}
