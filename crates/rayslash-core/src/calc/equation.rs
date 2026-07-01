use super::{
    error::CalcError,
    format_result,
    parser::{
        apply_function, has_invalid_operator_sequence, incomplete_rhs, superscript_digit_value,
    },
};

pub(super) fn solve_equation(expression: &str) -> Result<String, CalcError> {
    let mut parts = expression.split('=');
    let lhs = parts.next().unwrap_or_default().trim();
    let rhs = parts.next().ok_or(CalcError::MissingEquationSide)?.trim();

    if parts.next().is_some() {
        return Err(CalcError::TooManyEquals);
    }

    if lhs.is_empty() || rhs.is_empty() {
        return Err(CalcError::MissingEquationSide);
    }

    if has_invalid_operator_sequence(lhs) || has_invalid_operator_sequence(rhs) {
        return Err(CalcError::UnexpectedToken('+'));
    }

    let lhs = parse_linear_expression(lhs)?;
    let rhs = parse_linear_expression(rhs)?;
    let coefficient = lhs.x - rhs.x;
    let constant = rhs.constant - lhs.constant;

    if nearly_zero(coefficient) {
        return if nearly_zero(constant) {
            Ok("True".to_owned())
        } else {
            Err(CalcError::NoSolution)
        };
    }

    let value = constant / coefficient;
    if value.is_finite() {
        Ok(format!("x = {}", format_result(value)))
    } else {
        Err(CalcError::NonFiniteResult)
    }
}

fn parse_linear_expression(input: &str) -> Result<LinearValue, CalcError> {
    let mut parser = LinearParser::new(input);
    let value = parser.parse_expression()?;
    parser.skip_whitespace();

    if let Some(ch) = parser.peek() {
        return Err(CalcError::UnexpectedToken(ch));
    }

    Ok(value)
}

fn nearly_zero(value: f64) -> bool {
    value.abs() < 1e-10
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LinearValue {
    x: f64,
    constant: f64,
}

impl LinearValue {
    fn constant(value: f64) -> Self {
        Self {
            x: 0.0,
            constant: value,
        }
    }

    fn variable() -> Self {
        Self {
            x: 1.0,
            constant: 0.0,
        }
    }

    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            constant: self.constant + rhs.constant,
        }
    }

    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            constant: self.constant - rhs.constant,
        }
    }

    fn neg(self) -> Self {
        Self {
            x: -self.x,
            constant: -self.constant,
        }
    }

    fn mul(self, rhs: Self) -> Result<Self, CalcError> {
        if !nearly_zero(self.x) && !nearly_zero(rhs.x) {
            return Err(CalcError::NonlinearEquation);
        }

        Ok(Self {
            x: self.x * rhs.constant + rhs.x * self.constant,
            constant: self.constant * rhs.constant,
        })
    }

    fn div(self, rhs: Self) -> Result<Self, CalcError> {
        if !nearly_zero(rhs.x) {
            return Err(CalcError::NonlinearEquation);
        }

        if nearly_zero(rhs.constant) {
            return Err(CalcError::DivisionByZero);
        }

        Ok(Self {
            x: self.x / rhs.constant,
            constant: self.constant / rhs.constant,
        })
    }

    fn pow(self, rhs: Self) -> Result<Self, CalcError> {
        if !nearly_zero(rhs.x) {
            return Err(CalcError::NonlinearEquation);
        }

        if nearly_zero(self.x) {
            let value = self.constant.powf(rhs.constant);
            return if value.is_finite() {
                Ok(Self::constant(value))
            } else {
                Err(CalcError::NonFiniteResult)
            };
        }

        if nearly_zero(rhs.constant - 1.0) {
            Ok(self)
        } else if nearly_zero(rhs.constant) {
            Ok(Self::constant(1.0))
        } else {
            Err(CalcError::NonlinearEquation)
        }
    }
}

struct LinearParser<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> LinearParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, position: 0 }
    }

    fn parse_expression(&mut self) -> Result<LinearValue, CalcError> {
        self.parse_addition()
    }

    fn parse_addition(&mut self) -> Result<LinearValue, CalcError> {
        let mut value = self.parse_multiplication()?;

        loop {
            self.skip_whitespace();

            if self.consume('+') {
                value = value.add(self.parse_multiplication().map_err(incomplete_rhs)?);
            } else if self.consume('-') {
                value = value.sub(self.parse_multiplication().map_err(incomplete_rhs)?);
            } else {
                break;
            }
        }

        Ok(value)
    }

    fn parse_multiplication(&mut self) -> Result<LinearValue, CalcError> {
        let mut value = self.parse_unary()?;

        loop {
            self.skip_whitespace();

            if self.consume('*') {
                value = value.mul(self.parse_unary().map_err(incomplete_rhs)?)?;
            } else if self.consume('/') {
                value = value.div(self.parse_unary().map_err(incomplete_rhs)?)?;
            } else if self.starts_implicit_multiplication() {
                value = value.mul(self.parse_unary()?)?;
            } else {
                break;
            }
        }

        Ok(value)
    }

    fn parse_unary(&mut self) -> Result<LinearValue, CalcError> {
        self.skip_whitespace();

        if self.consume('+') {
            self.parse_unary().map_err(incomplete_rhs)
        } else if self.consume('-') {
            Ok(self.parse_unary().map_err(incomplete_rhs)?.neg())
        } else {
            self.parse_power()
        }
    }

    fn parse_power(&mut self) -> Result<LinearValue, CalcError> {
        let mut base = self.parse_primary()?;
        self.skip_whitespace();

        if self.consume_power_operator() {
            base = base.pow(self.parse_unary().map_err(incomplete_rhs)?)?;
        } else if let Some(exponent) = self.consume_superscript_exponent() {
            base = base.pow(LinearValue::constant(exponent))?;
        }

        Ok(base)
    }

    fn parse_primary(&mut self) -> Result<LinearValue, CalcError> {
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
            self.parse_number().map(LinearValue::constant)
        }
    }

    fn parse_identifier(&mut self) -> Result<LinearValue, CalcError> {
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

            if !nearly_zero(value.x) {
                return Err(CalcError::NonlinearEquation);
            }

            return apply_function(&identifier, value.constant).map(LinearValue::constant);
        }

        match identifier.as_str() {
            "x" => Ok(LinearValue::variable()),
            "pi" => Ok(LinearValue::constant(std::f64::consts::PI)),
            "e" => Ok(LinearValue::constant(std::f64::consts::E)),
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

    fn peek(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn advance(&mut self, ch: char) {
        self.position += ch.len_utf8();
    }
}
