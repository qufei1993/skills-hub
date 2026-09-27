use std::path::PathBuf;

use clap::{ArgAction, ArgGroup, Args, ColorChoice, Parser, Subcommand, ValueEnum};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Language {
    #[value(name = "en")]
    En,
    #[value(name = "zh-CN")]
    ZhCn,
    #[value(name = "ko")]
    Ko,
}

impl Language {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "en" => Some(Self::En),
            "zh-CN" => Some(Self::ZhCn),
            "ko" => Some(Self::Ko),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Parser)]
#[command(
    name = "skillshub-cli",
    version,
    about = "Manage Skills Hub from an agent or terminal",
    color = ColorChoice::Never,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub json: bool,

    #[arg(long, global = true, value_enum)]
    pub lang: Option<Language>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    Skills(SkillsArgs),
    Agents(AgentsArgs),
    Doctor,
    Version,
    Setup(SetupArgs),
    #[command(name = "__bridge", hide = true)]
    Bridge(BridgeArgs),
}

impl Command {
    pub const fn protocol_name(&self) -> &'static str {
        match self {
            Self::Skills(args) => args.command.protocol_name(),
            Self::Agents(args) => args.command.protocol_name(),
            Self::Doctor => "doctor",
            Self::Version => "version",
            Self::Setup(_) => "setup",
            Self::Bridge(args) => args.command.protocol_name(),
        }
    }
}

#[derive(Clone, Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: SkillsCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum SkillsCommand {
    List(ListArgs),
    Show(SkillSelectorArgs),
    Search(SearchArgs),
    Status(SkillSelectorArgs),
    Check(SelectorOrAllArgs),
    Install(InstallArgs),
    Deploy(DeploymentArgs),
    Undeploy(DeploymentArgs),
    Update(SelectorOrAllArgs),
    Adopt(AdoptArgs),
    Tag(TagArgs),
    Remove(RemoveArgs),
}

impl SkillsCommand {
    pub const fn protocol_name(&self) -> &'static str {
        match self {
            Self::List(_) => "skills.list",
            Self::Show(_) => "skills.show",
            Self::Search(_) => "skills.search",
            Self::Status(_) => "skills.status",
            Self::Check(_) => "skills.check",
            Self::Install(_) => "skills.install",
            Self::Deploy(_) => "skills.deploy",
            Self::Undeploy(_) => "skills.undeploy",
            Self::Update(_) => "skills.update",
            Self::Adopt(_) => "skills.adopt",
            Self::Tag(args) => args.command.protocol_name(),
            Self::Remove(_) => "skills.remove",
        }
    }
}

#[derive(Clone, Debug, Default, Args)]
pub struct ListArgs {
    #[arg(long, action = ArgAction::Append)]
    pub tag: Vec<String>,

    #[arg(long, action = ArgAction::Append)]
    pub source: Vec<String>,

    #[arg(long, action = ArgAction::Append)]
    pub agent: Vec<String>,

    #[arg(long, conflicts_with = "tag")]
    pub untagged: bool,

    #[arg(long)]
    pub status: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct SkillSelectorArgs {
    pub skill: String,
}

#[derive(Clone, Debug, Args)]
pub struct SearchArgs {
    pub query: String,

    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Args)]
#[command(group(
    ArgGroup::new("selection")
        .required(true)
        .multiple(false)
        .args(["skill", "all"])
))]
pub struct SelectorOrAllArgs {
    pub skill: Option<String>,

    #[arg(long)]
    pub all: bool,
}

#[derive(Clone, Debug, Args)]
pub struct InstallArgs {
    pub source: String,

    #[arg(long)]
    pub subpath: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct DeploymentArgs {
    pub skill: String,

    #[arg(long, required = true, action = ArgAction::Append)]
    pub agent: Vec<String>,

    #[arg(long)]
    pub project: Option<PathBuf>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Args)]
pub struct AdoptArgs {
    pub source: PathBuf,

    #[command(flatten)]
    pub safety: PreviewOrConfirm,
}

#[derive(Clone, Debug, Default, Args)]
pub struct PreviewOrConfirm {
    #[arg(long, conflicts_with = "yes")]
    pub dry_run: bool,

    #[arg(long, conflicts_with = "dry_run")]
    pub yes: bool,
}

#[derive(Clone, Debug, Args)]
pub struct TagArgs {
    #[command(subcommand)]
    pub command: TagCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum TagCommand {
    Add(TagValuesArgs),
    Remove(TagValuesArgs),
    Set(TagValuesArgs),
    List(TagListArgs),
    Rename(TagRenameArgs),
    Delete(TagDeleteArgs),
}

impl TagCommand {
    pub const fn protocol_name(&self) -> &'static str {
        match self {
            Self::Add(_) => "skills.tag.add",
            Self::Remove(_) => "skills.tag.remove",
            Self::Set(_) => "skills.tag.set",
            Self::List(_) => "skills.tag.list",
            Self::Rename(_) => "skills.tag.rename",
            Self::Delete(_) => "skills.tag.delete",
        }
    }
}

#[derive(Clone, Debug, Args)]
pub struct TagValuesArgs {
    pub skill: String,

    #[arg(required = true, num_args = 1..)]
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Args)]
pub struct TagListArgs {
    pub skill: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct TagRenameArgs {
    pub old: String,
    pub new: String,
}

#[derive(Clone, Debug, Args)]
pub struct TagDeleteArgs {
    pub tag: String,

    #[command(flatten)]
    pub safety: PreviewOrConfirm,
}

#[derive(Clone, Debug, Args)]
pub struct RemoveArgs {
    pub skill: String,

    #[command(flatten)]
    pub safety: PreviewOrConfirm,
}

#[derive(Clone, Debug, Args)]
pub struct AgentsArgs {
    #[command(subcommand)]
    pub command: AgentsCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum AgentsCommand {
    List,
}

impl AgentsCommand {
    pub const fn protocol_name(&self) -> &'static str {
        match self {
            Self::List => "agents.list",
        }
    }
}

#[derive(Clone, Debug, Args)]
pub struct SetupArgs {
    #[arg(long, required = true)]
    pub agent: Vec<String>,

    #[arg(long)]
    pub remove: bool,

    #[arg(long, requires = "remove", conflicts_with = "dry_run")]
    pub yes: bool,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Args)]
pub struct BridgeArgs {
    #[command(subcommand)]
    pub command: BridgeCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum BridgeCommand {
    Status,
}

impl BridgeCommand {
    pub const fn protocol_name(&self) -> &'static str {
        match self {
            Self::Status => "__bridge.status",
        }
    }
}
