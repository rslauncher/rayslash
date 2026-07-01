use rayslash_core::calc::{self, Calculation};

fn value(query: &str) -> Option<String> {
    match calc::calculate(query) {
        Some(Calculation::Value { result, .. }) => Some(result),
        Some(Calculation::Error { message, .. }) => panic!("{query:?} errored: {message}"),
        None => None,
    }
}

fn error(query: &str) -> Option<String> {
    match calc::calculate(query) {
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
