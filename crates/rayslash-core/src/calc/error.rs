#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CalcError {
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
    pub(super) fn message(self) -> &'static str {
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
