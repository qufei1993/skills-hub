# Project Skills

[Chinese reference](../docs/PROJECT-SKILLS.zh.md)

The `skills/` directory contains versioned Skill source files. Contributors can import them through Skills Hub and deploy them to their preferred AI coding tools. AI assistance is optional; all contributions follow the same [project rules](../AGENTS.md) and [quality checks](../CONTRIBUTING.md#quality-checks).

## Contents

- The `dsh-*` directories contain the DSH Engineering workflows used for development, review, documentation, and verification. See the [workflow guide](../docs/AI-ENGINEERING.md#usage-order) to choose a Skill for a task.
- [manage-skills-hub](manage-skills-hub/SKILL.md) is the app's bundled Skill for managing a local Skill library. It is maintained in this repository and is not required for contributing code.

## Import and Use

1. Clone this repository. In the Skills Hub desktop app, choose **Import local folder** and select this checkout's `skills/` directory.
2. Select the `dsh-*` Skills you need. Review any existing same-name Skills and their sources before installing; do not overwrite unrelated or locally edited copies.
3. In installation settings, choose your enabled AI tools and deployment scope. Use this checkout as the project directory when the selected tool and platform support project scope. Otherwise, choose global scope explicitly if suitable.
4. Complete installation and check the synchronization results. Follow your AI tool's instructions to load the deployed Skills, then use the task names in the workflow guide.

Keep the checkout at its original path while using it as a local source. Pulling repository updates changes these source files; update the corresponding imported Skills through the desktop app to refresh library copies and deployed files. Existing installations sourced elsewhere keep their original source until you explicitly change their setup.

Tool deployment directories such as `.agents/skills/` and `.claude/skills/` remain local and ignored by Git. Commit source changes here, not personal deployments or links to local library paths.

## Maintain the DSH Copies

[dsh-engineering.source.json](dsh-engineering.source.json) identifies the imported DSH Engineering version, exact source commit, and included Skills. Each package retains its `LICENSE`, `NOTICE`, and `UPSTREAM.json` with the DeepSeek Harness source attribution. The imported packages retain their original Chinese instructions.

Maintain the generic workflows in DSH Engineering, then copy the complete selected packages from a reviewed source commit and update the source record together. Verify the copied bytes, required resources, and local links, and run the repository's required checks. Keep Skills Hub-specific rules in the project guidance rather than modifying these imported copies. Updating the copies does not rewrite project rules or automatically update contributors' installations.
