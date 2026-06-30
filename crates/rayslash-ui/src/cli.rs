#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliCommand {
    Run,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseArgsError {
    args: Vec<String>,
}

impl ParseArgsError {
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

pub fn parse_args(args: &[String]) -> Result<CliCommand, ParseArgsError> {
    match args {
        [] => Ok(CliCommand::Run),
        [arg] if arg == "toggle" => Ok(CliCommand::Toggle),
        _ => Err(ParseArgsError {
            args: args.to_vec(),
        }),
    }
}

pub fn usage(program: &str) -> String {
    format!("Usage: {program} [toggle]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_runs_gui() {
        assert_eq!(parse_args(&[]), Ok(CliCommand::Run));
    }

    #[test]
    fn toggle_arg_sends_toggle() {
        assert_eq!(parse_args(&["toggle".to_string()]), Ok(CliCommand::Toggle));
    }

    #[test]
    fn unknown_args_are_rejected() {
        let args = vec!["--help".to_string()];
        let error = parse_args(&args).expect_err("unknown args should fail");

        assert_eq!(error.args(), args);
    }

    #[test]
    fn extra_args_are_rejected() {
        let args = vec!["toggle".to_string(), "now".to_string()];

        assert!(parse_args(&args).is_err());
    }
}
