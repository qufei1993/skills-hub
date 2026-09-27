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
fn json_success_recursively_redacts_sensitive_payload_values() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        ["skillshub-cli", "--json", "doctor"],
        &mut stdout,
        &mut stderr,
        |_| {
            Ok(CommandSuccess::new(
                "doctor",
                json!({
                    "agent": "codex",
                    "path": "/tmp/safe-target",
                    "nested": {
                        "access_token": "success-token",
                        "Password": "success-password",
                        "client_secret": "success-client-secret",
                        "private-key": "-----BEGIN PRIVATE KEY-----",
                        "authorization": "Bearer success-authorization",
                        "Cookie": "session=success-cookie",
                        "code": "success-oauth-code"
                    },
                    "source": "https://alice:success-password@example.com/private.git?access_token=success-query#success-fragment",
                    "message": "fetch https://bob:success-password@example.com/repo.git?token=success-query#fragment failed",
                    "notes": ["safe note", "Bearer success-bearer", "request token=success-plain-token"]
                }),
                MessageKey::CommandCompleted,
            ))
        },
    );

    assert_eq!(exit, 0);
    assert!(stderr.is_empty());
    let payload = serde_json::from_slice::<Value>(&stdout).unwrap();
    assert_eq!(payload["data"]["agent"], "codex");
    assert_eq!(payload["data"]["path"], "/tmp/safe-target");
    for key in [
        "access_token",
        "Password",
        "client_secret",
        "private-key",
        "authorization",
        "Cookie",
        "code",
    ] {
        assert_eq!(payload["data"]["nested"][key], "[REDACTED]");
    }
    assert_eq!(payload["data"]["source"], "https://example.com/private.git");
    assert_eq!(
        payload["data"]["message"],
        "fetch https://example.com/repo.git failed"
    );
    assert_eq!(payload["data"]["notes"][0], "safe note");
    assert_eq!(payload["data"]["notes"][1], "[REDACTED]");
    assert_eq!(payload["data"]["notes"][2], "[REDACTED]");

    let serialized = String::from_utf8(stdout).unwrap();
    for secret in [
        "success-token",
        "success-password",
        "success-authorization",
        "success-client-secret",
        "success-cookie",
        "success-oauth-code",
        "success-query",
        "success-fragment",
        "success-bearer",
        "success-plain-token",
    ] {
        assert!(!serialized.contains(secret), "leaked {secret}");
    }
}

#[test]
fn sanitizer_handles_arbitrary_url_schemes_and_whitespace_auth_credentials() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        ["skillshub-cli", "--json", "doctor"],
        &mut stdout,
        &mut stderr,
        |_| {
            Ok(CommandSuccess::new(
                "doctor",
                json!({
                    "connections": [
                        "postgres://db-user:postgres-password@db.example/app?ssl=true#primary",
                        "mongodb://mongo-user:mongo-password@mongo.example/data?replicaSet=main",
                        "wss://socket-user:socket-password@socket.example/events?token=socket-query#live",
                        "custom://custom-user:custom-password@custom.example/resource?credential=custom-query#fragment"
                    ],
                    "malformed": "connect custom://[broken@host?secret=malformed-secret",
                    "headers": [
                        "Bearer\twhitespace-secret",
                        "Authorization=Bearer\tassignment-secret",
                        "Bearer:colon-secret",
                        "Basic=basic-password",
                        "Bearer abc",
                        "Basic YTpi",
                        "Authorization: Bearer abc"
                    ],
                    "description": "basic usage remains readable"
                }),
                MessageKey::CommandCompleted,
            ))
        },
    );

    assert_eq!(exit, 0);
    assert!(stderr.is_empty());
    let payload = serde_json::from_slice::<Value>(&stdout).unwrap();
    assert_eq!(
        payload["data"]["connections"],
        json!([
            "postgres://db.example/app",
            "mongodb://mongo.example/data",
            "wss://socket.example/events",
            "custom://custom.example/resource"
        ])
    );
    assert_eq!(payload["data"]["malformed"], "[REDACTED]");
    assert_eq!(payload["data"]["headers"][0], "[REDACTED]");
    assert_eq!(payload["data"]["headers"][1], "[REDACTED]");
    assert_eq!(payload["data"]["headers"][2], "[REDACTED]");
    assert_eq!(payload["data"]["headers"][3], "[REDACTED]");
    assert_eq!(payload["data"]["headers"][4], "[REDACTED]");
    assert_eq!(payload["data"]["headers"][5], "[REDACTED]");
    assert_eq!(payload["data"]["headers"][6], "[REDACTED]");
    assert_eq!(
        payload["data"]["description"],
        "basic usage remains readable"
    );

    let serialized = String::from_utf8(stdout).unwrap();
    for secret in [
        "postgres-password",
        "mongo-password",
        "socket-password",
        "custom-password",
        "malformed-secret",
        "whitespace-secret",
        "assignment-secret",
        "colon-secret",
        "basic-password",
    ] {
        assert!(!serialized.contains(secret), "leaked {secret}");
    }
}

#[test]
fn sanitizer_preserves_metadata_and_only_treats_code_as_secret_in_auth_context() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        ["skillshub-cli", "--json", "doctor"],
        &mut stdout,
        &mut stderr,
        |_| {
            Ok(CommandSuccess::new(
                "doctor",
                json!({
                    "code": 200,
                    "status": {"code": "READY"},
                    "metadata": {
                        "token_count": 42,
                        "password_policy": {"minimum_length": 12},
                        "authorization_status": false
                    },
                    "key_variants": {
                        "oauth_token": "oauth-token-secret",
                        "bearer_token": "bearer-token-secret",
                        "basic_auth": "basic-auth-secret",
                        "client_secret": "client-secret-value"
                    },
                    "candidates": [{"code": 409, "path": "/tmp/candidate", "reason": "conflict"}],
                    "plan_id": "plan-safe",
                    "source": "custom://safe.example/resource",
                    "auth": {
                        "code": "parent-context-secret",
                        "grant_type": "authorization_code",
                        "code_verifier": "verifier-secret",
                        "result": {"code": 201, "reason": "created"}
                    },
                    "flows": [
                        {"code": "NORMAL", "reason": "not-auth"},
                        {"oauth": false, "code": 200},
                        {"oauth": null, "code": 201},
                        {"oauth": 0, "code": 202},
                        {"oauth": "false", "code": 203},
                        {"oauth": true, "client_id": "public-client", "code": "sibling-context-secret"},
                        {"oauth": {"provider": "github"}, "code": "descriptor-context-secret"}
                    ]
                }),
                MessageKey::CommandCompleted,
            ))
        },
    );

    assert_eq!(exit, 0);
    assert!(stderr.is_empty());
    let payload = serde_json::from_slice::<Value>(&stdout).unwrap();
    assert_eq!(payload["data"]["code"], 200);
    assert_eq!(payload["data"]["status"]["code"], "READY");
    assert_eq!(payload["data"]["metadata"]["token_count"], 42);
    assert_eq!(
        payload["data"]["metadata"]["password_policy"],
        json!({"minimum_length": 12})
    );
    assert_eq!(payload["data"]["metadata"]["authorization_status"], false);
    assert_eq!(payload["data"]["key_variants"]["oauth_token"], "[REDACTED]");
    assert_eq!(
        payload["data"]["key_variants"]["bearer_token"],
        "[REDACTED]"
    );
    assert_eq!(payload["data"]["key_variants"]["basic_auth"], "[REDACTED]");
    assert_eq!(
        payload["data"]["key_variants"]["client_secret"],
        "[REDACTED]"
    );
    assert_eq!(payload["data"]["candidates"][0]["code"], 409);
    assert_eq!(payload["data"]["candidates"][0]["path"], "/tmp/candidate");
    assert_eq!(payload["data"]["candidates"][0]["reason"], "conflict");
    assert_eq!(payload["data"]["plan_id"], "plan-safe");
    assert_eq!(payload["data"]["source"], "custom://safe.example/resource");
    assert_eq!(payload["data"]["auth"]["code"], "[REDACTED]");
    assert_eq!(payload["data"]["auth"]["code_verifier"], "[REDACTED]");
    assert_eq!(payload["data"]["auth"]["result"]["code"], 201);
    assert_eq!(payload["data"]["auth"]["result"]["reason"], "created");
    assert_eq!(payload["data"]["flows"][0]["code"], "NORMAL");
    assert_eq!(payload["data"]["flows"][1]["code"], 200);
    assert_eq!(payload["data"]["flows"][2]["code"], 201);
    assert_eq!(payload["data"]["flows"][3]["code"], 202);
    assert_eq!(payload["data"]["flows"][4]["code"], 203);
    assert_eq!(payload["data"]["flows"][5]["oauth"], true);
    assert_eq!(payload["data"]["flows"][5]["client_id"], "public-client");
    assert_eq!(payload["data"]["flows"][5]["code"], "[REDACTED]");
    assert_eq!(payload["data"]["flows"][6]["code"], "[REDACTED]");

    let serialized = String::from_utf8(stdout).unwrap();
    assert!(!serialized.contains("parent-context-secret"));
    assert!(!serialized.contains("verifier-secret"));
    assert!(!serialized.contains("sibling-context-secret"));
    assert!(!serialized.contains("descriptor-context-secret"));
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
fn json_failure_redacts_details_without_changing_protocol_or_safe_recovery_fields() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        [
            "skillshub-cli",
            "--json",
            "skills",
            "deploy",
            "demo",
            "--agent",
            "codex",
        ],
        &mut stdout,
        &mut stderr,
        |_| {
            Err(ServiceError::new(
                ErrorCode::TargetConflict,
                "target conflict",
                json!({
                    "candidates": [{"id": "one", "name": "demo"}],
                    "agent": "codex",
                    "path": "/tmp/safe-target",
                    "reason": "unmanaged_target",
                    "bundled_rollback": {
                        "files_restored": true,
                        "recovery_path": "/tmp/.skills-hub-recovery-example",
                        "backup_path": "/tmp/.skills-hub-backup-example",
                        "reason": "concurrent_content_preserved"
                    },
                    "nested": {
                        "refreshToken": "failure-token",
                        "password": "failure-password",
                        "client-secret": "failure-client-secret",
                        "Authorization": "Bearer failure-authorization",
                        "cookie": "session=failure-cookie",
                        "code": "failure-oauth-code",
                        "repository": "https://user:failure-password@example.com/repo.git?oauth_code=failure-query#failure-fragment"
                    }
                }),
            ))
        },
    );

    assert_eq!(exit, 4);
    assert!(stdout.is_empty());
    let payload = serde_json::from_slice::<Value>(&stderr).unwrap();
    assert_eq!(payload["code"], "TARGET_CONFLICT");
    assert_eq!(payload["command"], "skills.deploy");
    assert_eq!(payload["details"]["candidates"][0]["name"], "demo");
    assert_eq!(payload["details"]["agent"], "codex");
    assert_eq!(payload["details"]["path"], "/tmp/safe-target");
    assert_eq!(payload["details"]["reason"], "unmanaged_target");
    assert_eq!(
        payload["details"]["bundled_rollback"]["recovery_path"],
        "/tmp/.skills-hub-recovery-example"
    );
    assert_eq!(
        payload["details"]["bundled_rollback"]["backup_path"],
        "/tmp/.skills-hub-backup-example"
    );
    assert_eq!(
        payload["details"]["bundled_rollback"]["files_restored"],
        true
    );
    assert_eq!(
        payload["details"]["bundled_rollback"]["reason"],
        "concurrent_content_preserved"
    );
    for key in [
        "refreshToken",
        "password",
        "client-secret",
        "Authorization",
        "cookie",
        "code",
    ] {
        assert_eq!(payload["details"]["nested"][key], "[REDACTED]");
    }
    assert_eq!(
        payload["details"]["nested"]["repository"],
        "https://example.com/repo.git"
    );

    let serialized = String::from_utf8(stderr).unwrap();
    for secret in [
        "failure-token",
        "failure-password",
        "failure-authorization",
        "failure-client-secret",
        "failure-cookie",
        "failure-oauth-code",
        "failure-query",
        "failure-fragment",
    ] {
        assert!(!serialized.contains(secret), "leaked {secret}");
    }
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
fn supported_environment_locale_drives_help_when_lang_is_absent() {
    let environment = |name: &str| match name {
        "LC_ALL" => Some(OsString::from("fr_FR.UTF-8")),
        "LC_MESSAGES" => Some(OsString::from("ko_KR.UTF-8")),
        "LANG" => Some(OsString::from("zh_CN.UTF-8")),
        _ => None,
    };
    let locale = Locale::resolve_with(None, environment);
    let error = locale
        .localize_command(Cli::command())
        .try_get_matches_from(["skillshub-cli", "--help"])
        .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::DisplayHelp);
    let help = error.to_string();
    assert!(help.contains("Agent 또는 터미널에서 Skills Hub를 관리합니다"));
    assert!(help.contains("사용법:"));
    assert!(!help.contains("__bridge"));
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
    let error = String::from_utf8(stderr).unwrap();
    assert!(error.starts_with("INVALID_ARGUMENT: 命令参数无效。\n"));
    assert!(error.contains("缺少必需参数：\n  --agent <AGENT>"));
    assert!(error.contains("用法：skillshub-cli skills deploy --agent <AGENT> <SKILL>"));
    assert!(!error.contains("\u{1b}["));
}

#[test]
fn top_level_help_is_localized_from_the_shared_command_definition() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        ["skillshub-cli", "--lang", "zh-CN", "--help"],
        &mut stdout,
        &mut stderr,
        |_| panic!("help must not reach the executor"),
    );

    assert_eq!(exit, 0);
    assert!(stderr.is_empty());
    let help = String::from_utf8(stdout).unwrap();
    assert!(help.contains("从 Agent 或终端管理 Skills Hub"));
    assert!(help.contains("用法："));
    assert!(help.contains("命令："));
    assert!(help.contains("选项："));
    assert!(help.contains("查找、安装、部署、更新、标记、导入或移除 Skill"));
    assert!(!help.contains("参数：\n\n"));
    assert!(!help.contains("__bridge"));
}

#[test]
fn deploy_help_is_localized_in_korean_with_argument_guidance() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit = run_with_executor(
        [
            "skillshub-cli",
            "--lang",
            "ko",
            "skills",
            "deploy",
            "--help",
        ],
        &mut stdout,
        &mut stderr,
        |_| panic!("help must not reach the executor"),
    );

    assert_eq!(exit, 0);
    assert!(stderr.is_empty());
    let help = String::from_utf8(stdout).unwrap();
    assert!(help.contains("Skill을 하나 이상의 명시적 Agent에 배포합니다"));
    assert!(help.contains("사용법:"));
    assert!(help.contains("인수:"));
    assert!(help.contains("옵션:"));
    assert!(help.contains("배포할 Skill 이름 또는 ID"));
    assert!(help.contains("대상 Agent를 하나 이상 지정"));
    assert!(!help.contains("\n명령:\n"));
    assert!(!help.contains("__bridge"));
}
