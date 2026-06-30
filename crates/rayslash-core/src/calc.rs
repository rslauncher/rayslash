#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Calculation {
    Value { expression: String, result: String },
    Error { expression: String, message: String },
}

pub fn calculate(query: &str) -> Option<Calculation> {
    let expression = query.trim();
    if expression.is_empty() {
        return None;
    }

    let has_calculation_hint = has_calculation_hint(expression);
    if !contains_only_math_chars(expression) {
        return has_calculation_hint.then(|| Calculation::Error {
            expression: expression.to_owned(),
            message: CalcError::UnsupportedCharacter.message().to_owned(),
        });
    }

    if expression.contains('=') {
        return Some(match solve_equation(expression) {
            Ok(result) => Calculation::Value {
                expression: expression.to_owned(),
                result,
            },
            Err(error) => Calculation::Error {
                expression: expression.to_owned(),
                message: error.message().to_owned(),
            },
        });
    }

    if has_invalid_operator_sequence(expression) {
        return Some(Calculation::Error {
            expression: expression.to_owned(),
            message: CalcError::UnexpectedToken('+').message().to_owned(),
        });
    }

    let mut parser = Parser::new(expression);
    let value = match parser.parse_expression() {
        Ok(value) => value,
        Err(error) => {
            return has_calculation_hint.then(|| Calculation::Error {
                expression: expression.to_owned(),
                message: error.message().to_owned(),
            });
        }
    };
    parser.skip_whitespace();

    if let Some(ch) = parser.peek() {
        return has_calculation_hint.then(|| Calculation::Error {
            expression: expression.to_owned(),
            message: CalcError::UnexpectedToken(ch).message().to_owned(),
        });
    }

    if !parser.saw_calculation_signal() {
        return None;
    }

    if !value.is_finite() {
        return Some(Calculation::Error {
            expression: expression.to_owned(),
            message: CalcError::NonFiniteResult.message().to_owned(),
        });
    }

    Some(Calculation::Value {
        expression: expression.to_owned(),
        result: format_result(value),
    })
}

fn has_calculation_hint(expression: &str) -> bool {
    let has_digit = expression.chars().any(|ch| ch.is_ascii_digit());
    if has_digit
        && expression
            .chars()
            .any(|ch| !ch.is_ascii_alphanumeric() && !ch.is_ascii_whitespace())
    {
        return true;
    }

    if expression.chars().any(|ch| {
        matches!(ch, '+' | '-' | '*' | '/' | '^' | '=' | '(' | ')' | '.')
            || superscript_digit_value(ch).is_some()
            || matches!(ch, '⁺' | '⁻')
    }) {
        return true;
    }

    let mut previous = None;
    let mut previous_non_space = None;
    for ch in expression.chars() {
        if previous.is_some_and(|previous: char| {
            (previous.is_ascii_digit() && ch.is_ascii_alphabetic())
                || (previous.is_ascii_alphabetic() && ch.is_ascii_digit())
        }) {
            return true;
        }
        if ch.is_ascii_digit()
            && previous_non_space.is_some_and(|previous: char| previous.is_ascii_digit())
            && previous != previous_non_space
        {
            return true;
        }
        if !ch.is_ascii_whitespace() {
            previous_non_space = Some(ch);
        }
        previous = Some(ch);
    }

    false
}

fn contains_only_math_chars(expression: &str) -> bool {
    expression.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || superscript_digit_value(ch).is_some()
            || matches!(
                ch,
                '+' | '-' | '*' | '/' | '^' | '=' | '(' | ')' | '.' | ' ' | '\t' | '⁺' | '⁻'
            )
    })
}

fn has_invalid_operator_sequence(expression: &str) -> bool {
    expression.contains("++")
}

fn format_result(value: f64) -> String {
    let value = if value == -0.0 { 0.0 } else { value };
    let rounded = value.round();
    if (value - rounded).abs() < 1e-10 && rounded >= i64::MIN as f64 && rounded <= i64::MAX as f64 {
        return (rounded as i64).to_string();
    }

    let mut result = if value.abs() >= 1e12 || (value != 0.0 && value.abs() < 1e-9) {
        format!("{value:.12e}")
    } else {
        format!("{value:.12}")
    };

    if let Some((mantissa, exponent)) = result.split_once('e') {
        let mantissa = trim_decimal(mantissa);
        let exponent = trim_exponent(exponent);
        result = format!("{mantissa}e{exponent}");
    } else {
        result = trim_decimal(&result);
    }

    if result == "-0" {
        "0".to_owned()
    } else {
        result
    }
}

fn trim_decimal(value: &str) -> String {
    value.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn trim_exponent(value: &str) -> String {
    let negative = value.starts_with('-');
    let digits = value.trim_start_matches(['+', '-']).trim_start_matches('0');

    let digits = if digits.is_empty() { "0" } else { digits };
    if negative && digits != "0" {
        format!("-{digits}")
    } else {
        digits.to_owned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalcError {
    UnsupportedCharacter,
    UnexpectedToken(char),
    ExpectedNumber,
    IncompleteExpression,
    MissingClosingParenthesis,
    DivisionByZero,
    UnknownFunction,
    UnknownIdentifier,
    DomainError,
    NonFiniteResult,
    MissingEquationSide,
    TooManyEquals,
    NonlinearEquation,
    NoSolution,
}

impl CalcError {
    fn message(self) -> &'static str {
        match self {
            CalcError::UnsupportedCharacter => "This calculation uses an unsupported character.",
            CalcError::UnexpectedToken(_) => "This calculation has an unexpected value.",
            CalcError::ExpectedNumber => "Type a number here to finish the calculation.",
            CalcError::IncompleteExpression => "Finish the expression to calculate it.",
            CalcError::MissingClosingParenthesis => {
                "Add a closing parenthesis to finish this calculation."
            }
            CalcError::DivisionByZero => "Division by zero is not possible.",
            CalcError::UnknownFunction => "That function is not supported yet.",
            CalcError::UnknownIdentifier => "That value is not supported in calculations.",
            CalcError::DomainError => "This calculation is outside the function's valid range.",
            CalcError::NonFiniteResult => {
                "This calculation is too large to show as a finite result."
            }
            CalcError::MissingEquationSide => "Both sides of the equation need a value.",
            CalcError::TooManyEquals => "Use only one equals sign in an equation.",
            CalcError::NonlinearEquation => "Only linear equations in x are supported.",
            CalcError::NoSolution => "This equation has no solution.",
        }
    }
}

fn solve_equation(expression: &str) -> Result<String, CalcError> {
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

struct Parser<'a> {
    input: &'a str,
    position: usize,
    binary_operator_count: usize,
    function_count: usize,
    implicit_multiplication_count: usize,
    superscript_exponent_count: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            position: 0,
            binary_operator_count: 0,
            function_count: 0,
            implicit_multiplication_count: 0,
            superscript_exponent_count: 0,
        }
    }

    fn parse_expression(&mut self) -> Result<f64, CalcError> {
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

    fn saw_calculation_signal(&self) -> bool {
        self.binary_operator_count > 0
            || self.function_count > 0
            || self.implicit_multiplication_count > 0
            || self.superscript_exponent_count > 0
    }
}

fn incomplete_rhs(error: CalcError) -> CalcError {
    match error {
        CalcError::ExpectedNumber => CalcError::IncompleteExpression,
        error => error,
    }
}

fn apply_function(name: &str, value: f64) -> Result<f64, CalcError> {
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

fn superscript_digit_value(ch: char) -> Option<u8> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn value(query: &str) -> Option<String> {
        match calculate(query) {
            Some(Calculation::Value { result, .. }) => Some(result),
            Some(Calculation::Error { message, .. }) => panic!("{query:?} errored: {message}"),
            None => None,
        }
    }

    fn error(query: &str) -> Option<String> {
        match calculate(query) {
            Some(Calculation::Error { message, .. }) => Some(message),
            Some(Calculation::Value { result, .. }) => panic!("{query:?} solved: {result}"),
            None => None,
        }
    }

    #[test]
    fn detects_simple_math_expressions() {
        assert_eq!(value("1+2"), Some("3".to_owned()));
        assert_eq!(value("10 - 4"), Some("6".to_owned()));
        assert_eq!(value("2 * 3"), Some("6".to_owned()));
        assert_eq!(value("8 / 2"), Some("4".to_owned()));
        assert_eq!(value("0.5 + 1.25"), Some("1.75".to_owned()));
    }

    #[test]
    fn respects_precedence_parentheses_and_unary_signs() {
        assert_eq!(value("2 + 3 * 4"), Some("14".to_owned()));
        assert_eq!(value("(2 + 3) * 4"), Some("20".to_owned()));
        assert_eq!(value("-(2 + 3) * 4"), Some("-20".to_owned()));
        assert_eq!(value("-2^2"), Some("-4".to_owned()));
        assert_eq!(value("(-2)^2"), Some("4".to_owned()));
    }

    #[test]
    fn supports_exponents_in_ascii_and_superscript_forms() {
        assert_eq!(value("2**2"), Some("4".to_owned()));
        assert_eq!(value("2^3^2"), Some("512".to_owned()));
        assert_eq!(value("10²"), Some("100".to_owned()));
        assert_eq!(value("10⁻²"), Some("0.01".to_owned()));
    }

    #[test]
    fn supports_constants_functions_and_implicit_multiplication() {
        assert_eq!(value("2(3 + 4)"), Some("14".to_owned()));
        assert_eq!(value("2pi"), Some("6.28318530718".to_owned()));
        assert_eq!(value("sqrt(81)"), Some("9".to_owned()));
        assert_eq!(value("log10(100)"), Some("2".to_owned()));
        assert_eq!(value("abs(-5) + round(2.4)"), Some("7".to_owned()));
        assert_eq!(value("floor(2.9) + ceil(2.1)"), Some("5".to_owned()));
    }

    #[test]
    fn solves_linear_equations_for_x() {
        assert_eq!(value("x+10/2=8"), Some("x = 3".to_owned()));
        assert_eq!(value("2x + 4 = 10"), Some("x = 3".to_owned()));
        assert_eq!(value("10/2 + x = 8"), Some("x = 3".to_owned()));
        assert_eq!(value("x / 2 = 4"), Some("x = 8".to_owned()));
        assert_eq!(value("2(x + 3) = 10"), Some("x = 2".to_owned()));
        assert_eq!(value("x + pi = 2pi"), Some("x = 3.14159265359".to_owned()));
    }

    #[test]
    fn handles_constant_equations() {
        assert_eq!(value("2 + 2 = 4"), Some("True".to_owned()));
        assert_eq!(
            error("2 + 2 = 5"),
            Some("This equation has no solution.".to_owned())
        );
    }

    #[test]
    fn does_not_treat_plain_queries_or_plain_values_as_calculations() {
        assert_eq!(value(""), None);
        assert_eq!(value("code"), None);
        assert_eq!(value("calculator"), None);
        assert_eq!(value("pi"), None);
        assert_eq!(value("42"), None);
    }

    #[test]
    fn reports_division_by_zero() {
        assert_eq!(
            error("10 / 0"),
            Some("Division by zero is not possible.".to_owned())
        );
    }

    #[test]
    fn reports_incomplete_expressions() {
        assert_eq!(
            error("1 +"),
            Some("Finish the expression to calculate it.".to_owned())
        );
        assert_eq!(
            error("2^"),
            Some("Finish the expression to calculate it.".to_owned())
        );
        assert_eq!(
            error("10+/2"),
            Some("Finish the expression to calculate it.".to_owned())
        );
    }

    #[test]
    fn reports_repeated_operator_errors() {
        assert_eq!(
            error("10++2"),
            Some("This calculation has an unexpected value.".to_owned())
        );
    }

    #[test]
    fn reports_missing_parentheses() {
        assert_eq!(
            error("(1 + 2"),
            Some("Add a closing parenthesis to finish this calculation.".to_owned())
        );
        assert_eq!(
            error("sqrt(9"),
            Some("Add a closing parenthesis to finish this calculation.".to_owned())
        );
    }

    #[test]
    fn reports_invalid_numbers_and_unexpected_tokens() {
        assert_eq!(
            error("1..2 + 3"),
            Some("This calculation has an unexpected value.".to_owned())
        );
        assert_eq!(
            error("2 3"),
            Some("This calculation has an unexpected value.".to_owned())
        );
    }

    #[test]
    fn reports_unknown_identifiers_and_functions() {
        assert_eq!(
            error("2foo"),
            Some("That value is not supported in calculations.".to_owned())
        );
        assert_eq!(
            error("unknown(2)"),
            Some("That function is not supported yet.".to_owned())
        );
    }

    #[test]
    fn reports_function_domain_and_non_finite_errors() {
        assert_eq!(
            error("sqrt(-1)"),
            Some("This calculation is outside the function's valid range.".to_owned())
        );
        assert_eq!(
            error("10^10000"),
            Some("This calculation is too large to show as a finite result.".to_owned())
        );
    }

    #[test]
    fn reports_equation_errors() {
        assert_eq!(
            error("x^2 = 4"),
            Some("Only linear equations in x are supported.".to_owned())
        );
        assert_eq!(
            error("1 / x = 2"),
            Some("Only linear equations in x are supported.".to_owned())
        );
        assert_eq!(
            error("x + 1 ="),
            Some("Both sides of the equation need a value.".to_owned())
        );
        assert_eq!(
            error("x = 1 = 2"),
            Some("Use only one equals sign in an equation.".to_owned())
        );
    }

    #[test]
    fn reports_unsupported_characters_only_for_math_like_queries() {
        assert_eq!(
            error("2 $ 2"),
            Some("This calculation uses an unsupported character.".to_owned())
        );
        assert_eq!(error("hello!"), None);
    }
}
