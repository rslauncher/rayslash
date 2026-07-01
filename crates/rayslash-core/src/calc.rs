mod equation;
mod error;
mod parser;

use equation::solve_equation;
use error::CalcError;
use parser::{Parser, has_invalid_operator_sequence, superscript_digit_value};

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

pub(super) fn format_result(value: f64) -> String {
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
