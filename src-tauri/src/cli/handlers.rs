use serde::Serialize;
use serde_json::json;

use super::args::{
    Cli, Command, DeploymentArgs, PreviewOrConfirm, SelectorOrAllArgs, SkillsCommand, TagCommand,
};
use super::locale::MessageKey;
use super::output::CommandSuccess;
use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::services::deployment::{DeploymentRequest, DeploymentScope};
use crate::services::error::ServiceError;
use crate::services::install::InstallRequest;
use crate::services::library::{TagAction, TagSelector};
use crate::services::skills_hub::SkillsHubService;
use crate::services::workflows::{Confirmation, SkillFilter, SkillSelection};

pub fn execute(cli: &Cli) -> Result<CommandSuccess, ServiceError> {
    let command = cli.command.protocol_name();
    match &cli.command {
        Command::Version => return success(command, json!({"version": env!("CARGO_PKG_VERSION")})),
        Command::Setup(_) | Command::Bridge(_) => {
            return Err(ServiceError::internal(
                "CLI workflow handler is not available",
            ))
        }
        _ => {}
    }
    let service = SkillsHubService::open(runtime_paths()?)?;
    match &cli.command {
        Command::Doctor => success(command, service.doctor()?),
        Command::Agents(_) => success(command, service.list_agents()?),
        Command::Skills(args) => match &args.command {
            SkillsCommand::List(args) => success(
                command,
                service.filtered_skills(SkillFilter {
                    tags: args.tag.clone(),
                    sources: args.source.clone(),
                    agents: args.agent.clone(),
                    untagged: args.untagged,
                    status: args.status.clone(),
                })?,
            ),
            SkillsCommand::Show(args) => {
                success(command, service.show_skill(args.skill.clone().into())?)
            }
            SkillsCommand::Status(args) => {
                success(command, service.skill_status(args.skill.clone().into())?)
            }
            SkillsCommand::Search(args) => success(
                command,
                service.search(&args.query, args.limit.unwrap_or(20) as usize)?,
            ),
            SkillsCommand::Check(args) => {
                success(command, service.check_selection(selection(args)?)?)
            }
            SkillsCommand::Update(args) => {
                success(command, service.update_selection(selection(args)?)?)
            }
            SkillsCommand::Install(args) => {
                let mut request = InstallRequest::parse(&args.source)?;
                request.subpath = args.subpath.clone();
                success(command, service.install(request)?)
            }
            SkillsCommand::Deploy(args) => success(
                command,
                service.deploy_workflow(deployment(args), args.dry_run)?,
            ),
            SkillsCommand::Undeploy(args) => success(
                command,
                service.undeploy_workflow(deployment(args), args.dry_run)?,
            ),
            SkillsCommand::Adopt(args) => success(
                command,
                service.adopt_workflow(args.source.clone(), confirmation(&args.safety))?,
            ),
            SkillsCommand::Remove(args) => success(
                command,
                service.remove_workflow(args.skill.clone().into(), confirmation(&args.safety))?,
            ),
            SkillsCommand::Tag(args) => match &args.command {
                TagCommand::Add(args) => success(
                    command,
                    service.apply_tag_action(TagAction::Add {
                        skill: args.skill.clone().into(),
                        tags: args.tags.clone(),
                    })?,
                ),
                TagCommand::Remove(args) => success(
                    command,
                    service.apply_tag_action(TagAction::Remove {
                        skill: args.skill.clone().into(),
                        tags: args.tags.clone(),
                    })?,
                ),
                TagCommand::Set(args) => success(
                    command,
                    service.apply_tag_action(TagAction::Set {
                        skill: args.skill.clone().into(),
                        tags: args.tags.clone(),
                    })?,
                ),
                TagCommand::List(args) => {
                    success(command, service.tags(args.skill.clone().map(Into::into))?)
                }
                TagCommand::Rename(args) => success(
                    command,
                    service.apply_tag_action(TagAction::Rename {
                        tag: TagSelector::Name(args.old.clone()),
                        name: args.new.clone(),
                    })?,
                ),
                TagCommand::Delete(args) => success(
                    command,
                    service.delete_tag_workflow(
                        TagSelector::Name(args.tag.clone()),
                        confirmation(&args.safety),
                    )?,
                ),
            },
        },
        Command::Version | Command::Setup(_) | Command::Bridge(_) => unreachable!(),
    }
}

fn runtime_paths() -> Result<RuntimePaths, ServiceError> {
    #[cfg(debug_assertions)]
    if let Some(root) = std::env::var_os("SKILLSHUB_CLI_TEST_ROOT") {
        let root = std::path::PathBuf::from(root);
        if !root.is_absolute() || !root.is_dir() {
            return Err(ServiceError::internal("invalid CLI test root"));
        }
        return Ok(RuntimePaths::from_roots(
            RuntimeProfile::Test,
            root.join("home"),
            root.join("data"),
        ));
    }
    RuntimePaths::for_cli(default_runtime_profile())
        .map_err(|_| ServiceError::internal("failed to resolve runtime paths"))
}

fn default_runtime_profile() -> RuntimeProfile {
    RuntimeProfile::current()
}

fn selection(args: &SelectorOrAllArgs) -> Result<SkillSelection, ServiceError> {
    if args.all {
        Ok(SkillSelection::All)
    } else {
        args.skill
            .clone()
            .map(|skill| SkillSelection::One(skill.into()))
            .ok_or_else(|| ServiceError::internal("missing parsed skill selection"))
    }
}

fn deployment(args: &DeploymentArgs) -> DeploymentRequest {
    let mut request = DeploymentRequest::global(args.skill.clone(), args.agent.clone());
    request.scope = args
        .project
        .clone()
        .map(DeploymentScope::Project)
        .unwrap_or_default();
    request
}

fn confirmation(args: &PreviewOrConfirm) -> Confirmation {
    Confirmation {
        dry_run: args.dry_run,
        confirmed: args.yes,
    }
}

fn success(command: &'static str, data: impl Serialize) -> Result<CommandSuccess, ServiceError> {
    serde_json::to_value(data)
        .map(|data| CommandSuccess::new(command, data, MessageKey::CommandCompleted))
        .map_err(|_| ServiceError::internal("failed to serialize command result"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cli_paths_use_the_build_namespace_without_opening_directories() {
        let paths =
            RuntimePaths::from_roots(default_runtime_profile(), "/fixture/home", "/fixture/data");
        let (identifier, central, bridge) = if cfg!(debug_assertions) {
            (
                "com.qufei1993.skillshub.dev",
                ".skillshub-dev",
                ".skills-hub-dev",
            )
        } else {
            ("com.qufei1993.skillshub", ".skillshub", ".skills-hub")
        };
        assert_eq!(
            paths.app_data_dir,
            std::path::Path::new("/fixture/data").join(identifier)
        );
        assert_eq!(
            paths.database_path,
            std::path::Path::new("/fixture/data")
                .join(identifier)
                .join("skills_hub.db")
        );
        assert_eq!(
            paths.default_central_repo,
            std::path::Path::new("/fixture/home").join(central)
        );
        assert_eq!(
            paths.cli_bridge_dir,
            std::path::Path::new("/fixture/home")
                .join(bridge)
                .join("bin")
        );
    }
}
