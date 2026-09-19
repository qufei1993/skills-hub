use std::ffi::OsString;

use clap::{error::ErrorKind, CommandFactory, Parser};
use serde_json::{json, Value};

use super::args::{BridgeCommand, Cli, Command, SkillsCommand, TagCommand};
use super::locale::{Language, Locale, MessageKey};
use super::output::{exit_code_for_error, CommandSuccess};
use super::run_with_executor;
use crate::services::error::{ErrorCode, ServiceError};

#[test]
fn approved_command_tree_parses_every_public_command() {
    let cases: &[&[&str]] = &[
        &["skillshub-cli", "skills", "list"],
        &["skillshub-cli", "skills", "show", "demo"],
        &[
            "skillshub-cli",
            "skills",
            "search",
            "frontend",
            "--limit",
            "5",
        ],
        &["skillshub-cli", "skills", "status", "demo"],
        &["skillshub-cli", "skills", "check", "demo"],
        &["skillshub-cli", "skills", "check", "--all"],
        &["skillshub-cli", "skills", "install", "./demo"],
        &[
            "skillshub-cli",
            "skills",
            "deploy",
            "demo",
            "--agent",
            "codex",
            "--agent",
            "cursor",
            "--project",
            "/tmp/project",
            "--dry-run",
        ],
        &[
            "skillshub-cli",
            "skills",
            "undeploy",
            "demo",
            "--agent",
            "codex",
        ],
        &["skillshub-cli", "skills", "update", "demo"],
        &["skillshub-cli", "skills", "update", "--all"],
        &[
            "skillshub-cli",
            "skills",
            "adopt",
            "/tmp/agent-skills",
            "--dry-run",
        ],
        &[
            "skillshub-cli",
            "skills",
            "tag",
            "add",
            "demo",
            "frontend",
            "stable",
        ],
        &[
            "skillshub-cli",
            "skills",
            "tag",
            "remove",
            "demo",
            "frontend",
        ],
        &["skillshub-cli", "skills", "tag", "set", "demo", "frontend"],
        &["skillshub-cli", "skills", "tag", "list"],
        &["skillshub-cli", "skills", "tag", "list", "demo"],
        &["skillshub-cli", "skills", "tag", "rename", "old", "new"],
        &["skillshub-cli", "skills", "tag", "delete", "old", "--yes"],
        &["skillshub-cli", "skills", "remove", "demo", "--dry-run"],
        &["skillshub-cli", "agents", "list"],
        &["skillshub-cli", "doctor"],
        &["skillshub-cli", "version"],
        &["skillshub-cli", "setup", "--agent", "codex"],
        &[
            "skillshub-cli",
            "setup",
            "--agent",
            "codex",
            "--remove",
            "--yes",
        ],
    ];

    for args in cases {
        Cli::try_parse_from(*args).unwrap_or_else(|error| {
            panic!("failed to parse {args:?}: {error}");
        });
    }
}

#[test]
fn deploy_and_undeploy_require_an_explicit_agent() {
    for verb in ["deploy", "undeploy"] {
        let error = Cli::try_parse_from(["skillshub-cli", "skills", verb, "demo"]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }
}

#[test]
fn selector_or_all_is_required_and_mutually_exclusive() {
    for verb in ["check", "update"] {
        let missing = Cli::try_parse_from(["skillshub-cli", "skills", verb]).unwrap_err();
        assert_eq!(missing.kind(), ErrorKind::MissingRequiredArgument);

        let duplicate =
            Cli::try_parse_from(["skillshub-cli", "skills", verb, "demo", "--all"]).unwrap_err();
        assert_eq!(duplicate.kind(), ErrorKind::ArgumentConflict);
    }
}

#[test]
fn preview_and_confirmation_are_available_only_on_approved_commands() {
    for args in [
        ["skills", "adopt", "/tmp/skills", "--dry-run"].as_slice(),
        ["skills", "adopt", "/tmp/skills", "--yes"].as_slice(),
        ["skills", "remove", "demo", "--dry-run"].as_slice(),
        ["skills", "remove", "demo", "--yes"].as_slice(),
        ["skills", "tag", "delete", "old", "--dry-run"].as_slice(),
        ["skills", "tag", "delete", "old", "--yes"].as_slice(),
    ] {
        let mut argv = vec!["skillshub-cli"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).unwrap();
    }

    for args in [
        ["skills", "install", "./demo", "--dry-run"].as_slice(),
        ["skills", "update", "demo", "--yes"].as_slice(),
        ["skills", "tag", "add", "demo", "tag", "--dry-run"].as_slice(),
    ] {
        let mut argv = vec!["skillshub-cli"];
        argv.extend_from_slice(args);
        assert_eq!(
            Cli::try_parse_from(argv).unwrap_err().kind(),
            ErrorKind::UnknownArgument
        );
    }
}

#[test]
fn dry_run_and_yes_cannot_be_combined() {
    for args in [
        [
            "skillshub-cli",
            "skills",
            "adopt",
            "/tmp/skills",
            "--dry-run",
            "--yes",
        ]
        .as_slice(),
        [
            "skillshub-cli",
            "skills",
            "remove",
            "demo",
            "--dry-run",
            "--yes",
        ]
        .as_slice(),
        [
            "skillshub-cli",
            "skills",
            "tag",
            "delete",
            "old",
            "--dry-run",
            "--yes",
        ]
        .as_slice(),
    ] {
        assert_eq!(
            Cli::try_parse_from(args).unwrap_err().kind(),
            ErrorKind::ArgumentConflict
        );
    }
}

#[test]
fn install_never_accepts_implicit_deployment_arguments() {
    let error = Cli::try_parse_from([
        "skillshub-cli",
        "skills",
        "install",
        "./demo",
        "--agent",
        "codex",
    ])
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn excluded_surfaces_are_not_commands() {
    for command in ["sync", "preset", "pack", "config", "credentials"] {
        let error = Cli::try_parse_from(["skillshub-cli", command]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
    }
}

#[test]
fn bridge_contract_parses_but_is_absent_from_help() {
    let cli = Cli::try_parse_from(["skillshub-cli", "__bridge", "status"]).unwrap();
    assert!(matches!(
        cli.command,
        Command::Bridge(ref args) if matches!(args.command, BridgeCommand::Status)
    ));

    let mut help = Vec::new();
    Cli::command().write_long_help(&mut help).unwrap();
    let help = String::from_utf8(help).unwrap();
    assert!(!help.contains("__bridge"));
    assert!(!help.contains("bridge"));
}

#[test]
fn command_names_are_stable_protocol_identifiers() {
    let cli = Cli::try_parse_from([
        "skillshub-cli",
        "skills",
        "tag",
        "delete",
        "obsolete",
        "--dry-run",
    ])
    .unwrap();
    assert_eq!(cli.command.protocol_name(), "skills.tag.delete");

    let Command::Skills(skills) = cli.command else {
        panic!("expected skills command");
    };
    assert!(matches!(
        skills.command,
        SkillsCommand::Tag(ref tag) if matches!(tag.command, TagCommand::Delete(_))
    ));
}

#[test]
fn json_success_uses_stdout_only() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        ["skillshub-cli", "--json", "doctor"],
        &mut stdout,
        &mut stderr,
        |_| {
            Ok(CommandSuccess::new(
                "doctor",
                json!({"database_status": "ok"}),
                MessageKey::CommandCompleted,
            ))
        },
    );

    assert_eq!(exit, 0);
    assert!(stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&stdout).unwrap(),
        json!({
            "ok": true,
            "command": "doctor",
            "data": {"database_status": "ok"}
        })
    );
}

#[test]
fn json_failure_uses_stderr_only_and_skill_not_found_exits_three() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        [
            OsString::from("skillshub-cli"),
            OsString::from("--json"),
            OsString::from("--lang"),
            OsString::from("ko"),
            OsString::from("skills"),
            OsString::from("show"),
            OsString::from("missing"),
        ],
        &mut stdout,
        &mut stderr,
        |_| Err(ServiceError::skill_not_found("missing")),
    );

    assert_eq!(exit, 3);
    assert!(stdout.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&stderr).unwrap(),
        json!({
            "ok": false,
            "command": "skills.show",
            "code": "SKILL_NOT_FOUND",
            "message": "Skill을 찾을 수 없습니다.",
            "details": {"selector": "missing"}
        })
    );
}

#[test]
fn json_parse_failures_are_machine_readable_without_ansi() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        ["skillshub-cli", "--json", "skills", "deploy", "demo"],
        &mut stdout,
        &mut stderr,
        |_| panic!("invalid arguments must not reach the executor"),
    );

    assert_eq!(exit, 2);
    assert!(stdout.is_empty());
    let error = serde_json::from_slice::<Value>(&stderr).unwrap();
    assert_eq!(error["ok"], false);
    assert_eq!(error["command"], "skills.deploy");
    assert_eq!(error["code"], "INVALID_ARGUMENT");
    assert_eq!(error["details"]["kind"], "missing_required_argument");
    assert!(!String::from_utf8(stderr).unwrap().contains("\u{1b}["));
}

#[test]
fn service_messages_are_replaced_at_the_cli_localization_boundary() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        ["skillshub-cli", "--json", "doctor"],
        &mut stdout,
        &mut stderr,
        |_| {
            Err(ServiceError::new(
                ErrorCode::InternalError,
                "raw error chain must not escape",
                json!({}),
            ))
        },
    );

    assert_eq!(exit, 10);
    let text = String::from_utf8(stderr).unwrap();
    assert!(!text.contains("raw error chain"));
    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap()["message"],
        "Skills Hub could not complete the command."
    );
}

#[test]
fn stable_error_codes_have_the_specified_exit_mapping() {
    let cases = [
        (ErrorCode::InvalidArgument, 2),
        (ErrorCode::InvalidSource, 2),
        (ErrorCode::SkillNotFound, 3),
        (ErrorCode::AmbiguousSkill, 3),
        (ErrorCode::MultiSkills, 3),
        (ErrorCode::AgentNotFound, 3),
        (ErrorCode::ProjectScopeUnsupported, 4),
        (ErrorCode::TargetConflict, 4),
        (ErrorCode::UpdateHeldBack, 4),
        (ErrorCode::ConfirmationRequired, 4),
        (ErrorCode::PlanStale, 4),
        (ErrorCode::OperationBusy, 5),
        (ErrorCode::IncompatibleDatabase, 6),
        (ErrorCode::AuthRequired, 7),
        (ErrorCode::NetworkError, 7),
        (ErrorCode::InternalError, 10),
    ];

    for (code, expected) in cases {
        assert_eq!(exit_code_for_error(code), expected, "{code}");
    }
}

#[test]
fn explicit_language_precedes_supported_environment_then_english() {
    let environment = |name: &str| match name {
        "LC_ALL" => Some(OsString::from("ko_KR.UTF-8")),
        "LC_MESSAGES" => Some(OsString::from("zh_CN.UTF-8")),
        "LANG" => Some(OsString::from("en_US.UTF-8")),
        _ => None,
    };
    assert_eq!(
        Locale::resolve_with(Some(Language::ZhCn), environment),
        Locale::ZhCn
    );
    assert_eq!(Locale::resolve_with(None, environment), Locale::Ko);

    let unsupported = |_: &str| Some(OsString::from("fr_FR.UTF-8"));
    assert_eq!(Locale::resolve_with(None, unsupported), Locale::En);
}

#[test]
fn every_stable_error_has_complete_human_catalog_entries() {
    for locale in [Locale::En, Locale::ZhCn, Locale::Ko] {
        assert!(!locale.text(MessageKey::CommandCompleted).is_empty());
        assert!(!locale.text(MessageKey::InvalidArguments).is_empty());
        for code in ErrorCode::ALL {
            assert!(!locale.error_message(code).is_empty(), "{locale:?} {code}");
        }
    }
}

#[test]
fn human_failures_are_localized_and_never_contain_ansi() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        [
            "skillshub-cli",
            "--lang",
            "zh-CN",
            "skills",
            "show",
            "missing",
        ],
        &mut stdout,
        &mut stderr,
        |_| Err(ServiceError::skill_not_found("missing")),
    );

    assert_eq!(exit, 3);
    assert!(stdout.is_empty());
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "SKILL_NOT_FOUND: 未找到该 Skill。\n"
    );
}

#[test]
fn human_parser_failures_use_the_selected_locale() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        [
            "skillshub-cli",
            "--lang",
            "zh-CN",
            "skills",
            "deploy",
            "demo",
        ],
        &mut stdout,
        &mut stderr,
        |_| panic!("invalid arguments must not reach the executor"),
    );

    assert_eq!(exit, 2);
    assert!(stdout.is_empty());
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "INVALID_ARGUMENT: 命令参数无效。\n"
    );
}
