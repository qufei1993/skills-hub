pub mod args;
mod handlers;
pub mod locale;
pub mod output;
mod sanitize;

pub use args::Cli;
pub use output::JsonEnvelope;

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::process::ExitCode;

use clap::{
    error::{ContextKind, ContextValue, ErrorKind},
    CommandFactory, FromArgMatches,
};
use serde_json::json;

use self::locale::{Language, Locale, MessageKey};
use self::output::{write_failure, write_success, CommandSuccess};
use crate::services::error::{ErrorCode, ServiceError};

pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    ExitCode::from(run_with_io(args, &mut stdout, &mut stderr))
}

pub fn run_with_io<I, T, Stdout, Stderr>(args: I, stdout: &mut Stdout, stderr: &mut Stderr) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    Stdout: Write,
    Stderr: Write,
{
    run_with_executor(args, stdout, stderr, handlers::execute)
}

pub fn run_with_executor<I, T, Stdout, Stderr, Executor>(
    args: I,
    stdout: &mut Stdout,
    stderr: &mut Stderr,
    executor: Executor,
) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    Stdout: Write,
    Stderr: Write,
    Executor: FnOnce(&Cli) -> Result<CommandSuccess, ServiceError>,
{
    let raw_args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let json_output = requested_json(&raw_args);
    let locale = Locale::resolve(requested_language(&raw_args));
    let command_hint = protocol_command_hint(&raw_args);

    let cli = match locale
        .localize_command(Cli::command())
        .try_get_matches_from(raw_args)
        .and_then(|matches| Cli::from_arg_matches(&matches))
    {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            return if write!(stdout, "{error}").is_ok() {
                0
            } else {
                10
            };
        }
        Err(error) => {
            if json_output {
                let details = json!({ "kind": parser_error_kind(error.kind()) });
                let service_error = ServiceError::new(
                    ErrorCode::InvalidArgument,
                    "invalid command arguments",
                    details,
                );
                if write_failure(stderr, command_hint, &service_error, locale, true).is_err() {
                    return 10;
                }
            } else if write_human_parse_failure(stderr, locale, &error).is_err() {
                return 10;
            }
            return 2;
        }
    };

    match executor(&cli) {
        Ok(success) => {
            if write_success(stdout, &success, locale, cli.json).is_err() {
                10
            } else {
                0
            }
        }
        Err(error) => {
            let exit = output::exit_code_for_error(error.code);
            if write_failure(
                stderr,
                cli.command.protocol_name(),
                &error,
                locale,
                cli.json,
            )
            .is_err()
            {
                10
            } else {
                exit
            }
        }
    }
}

fn write_human_parse_failure(
    writer: &mut impl Write,
    locale: Locale,
    error: &clap::Error,
) -> std::io::Result<()> {
    writeln!(
        writer,
        "{}: {}",
        ErrorCode::InvalidArgument,
        locale.text(MessageKey::InvalidArguments)
    )?;

    if error.kind() == ErrorKind::MissingRequiredArgument {
        if let Some(ContextValue::Strings(arguments)) = error.get(ContextKind::InvalidArg) {
            let safe_arguments = arguments
                .iter()
                .filter(|argument| is_safe_usage_fragment(argument))
                .collect::<Vec<_>>();
            if !safe_arguments.is_empty() {
                writeln!(
                    writer,
                    "{}",
                    locale.text(MessageKey::MissingRequiredArguments)
                )?;
                for argument in safe_arguments {
                    writeln!(writer, "  {argument}")?;
                }
            }
        }
    }

    if let Some(ContextValue::StyledStr(usage)) = error.get(ContextKind::Usage) {
        let usage = usage.to_string();
        let usage = usage.strip_prefix("Usage: ").unwrap_or(&usage);
        if is_safe_usage_fragment(usage) {
            writeln!(writer, "{}{}", locale.text(MessageKey::Usage), usage)?;
        }
    }
    Ok(())
}

fn is_safe_usage_fragment(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(
                    character,
                    ' ' | '\n' | '\r' | '\t' | '-' | '_' | '<' | '>' | '[' | ']' | '|' | '.'
                )
        })
}

fn requested_json(args: &[OsString]) -> bool {
    args.iter().any(|argument| argument == OsStr::new("--json"))
}

fn requested_language(args: &[OsString]) -> Option<Language> {
    for (index, argument) in args.iter().enumerate() {
        let value = argument.to_string_lossy();
        if let Some(value) = value.strip_prefix("--lang=") {
            if let Some(language) = Language::parse(value) {
                return Some(language);
            }
        } else if value == "--lang" {
            if let Some(value) = args.get(index + 1).and_then(|value| value.to_str()) {
                if let Some(language) = Language::parse(value) {
                    return Some(language);
                }
            }
        }
    }
    None
}

fn protocol_command_hint(args: &[OsString]) -> &'static str {
    let values = args
        .iter()
        .filter_map(|value| value.to_str())
        .collect::<Vec<_>>();
    if let Some(index) = values.iter().position(|value| *value == "skills") {
        return match values.get(index + 1).copied() {
            Some("list") => "skills.list",
            Some("show") => "skills.show",
            Some("search") => "skills.search",
            Some("status") => "skills.status",
            Some("check") => "skills.check",
            Some("install") => "skills.install",
            Some("deploy") => "skills.deploy",
            Some("undeploy") => "skills.undeploy",
            Some("update") => "skills.update",
            Some("adopt") => "skills.adopt",
            Some("tag") => match values.get(index + 2).copied() {
                Some("add") => "skills.tag.add",
                Some("remove") => "skills.tag.remove",
                Some("set") => "skills.tag.set",
                Some("list") => "skills.tag.list",
                Some("rename") => "skills.tag.rename",
                Some("delete") => "skills.tag.delete",
                _ => "skills.tag",
            },
            Some("remove") => "skills.remove",
            _ => "skills",
        };
    }
    if let Some(index) = values.iter().position(|value| *value == "agents") {
        return if values.get(index + 1) == Some(&"list") {
            "agents.list"
        } else {
            "agents"
        };
    }
    if values.contains(&"doctor") {
        "doctor"
    } else if values.contains(&"version") {
        "version"
    } else if values.contains(&"setup") {
        "setup"
    } else if values.contains(&"__bridge") {
        "__bridge.status"
    } else {
        "cli"
    }
}

fn parser_error_kind(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::InvalidValue => "invalid_value",
        ErrorKind::UnknownArgument => "unknown_argument",
        ErrorKind::InvalidSubcommand => "invalid_subcommand",
        ErrorKind::NoEquals => "missing_equals",
        ErrorKind::ValueValidation => "value_validation",
        ErrorKind::TooManyValues => "too_many_values",
        ErrorKind::TooFewValues => "too_few_values",
        ErrorKind::WrongNumberOfValues => "wrong_number_of_values",
        ErrorKind::ArgumentConflict => "argument_conflict",
        ErrorKind::MissingRequiredArgument => "missing_required_argument",
        ErrorKind::MissingSubcommand => "missing_subcommand",
        ErrorKind::InvalidUtf8 => "invalid_utf8",
        ErrorKind::DisplayHelp => "display_help",
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => "display_help",
        ErrorKind::DisplayVersion => "display_version",
        ErrorKind::Io => "io",
        ErrorKind::Format => "format",
        _ => "invalid_argument",
    }
}

#[cfg(test)]
mod tests;
