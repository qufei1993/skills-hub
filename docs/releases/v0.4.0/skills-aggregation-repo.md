# 需求：Skills 聚合数据源升级 — 精选仓库列表方案

## 背景

SkillSyncer 应用的"探索"功能依赖 `featured-skills.json` 提供精选技能列表。早期方案通过 GitHub Search API 搜索 `claude-code-skill` 等 topic 标签自动发现仓库，但属于盲目搜索，质量不可控，噪音多。

**新方案**：维护一份精选仓库列表（Curated Repos），直接从已知的高质量仓库中获取 skills。质量可控、API 调用极少、维护简单。

## 目标

重写 `scripts/fetch-featured-skills.mjs` 脚本，从精选仓库列表直接获取元数据并深入检测 skill 目录结构，为每个 skill 生成独立条目，最终输出 `featured-skills.json`（最多 300 条）。保持现有的 GitHub Actions 定时更新机制不变。

## 架构（保持不变）

```
skills-desktop-app/
├── featured-skills.json                          # 精选技能数据（应用内嵌 fallback + 在线更新源）
├── scripts/
│   └── fetch-featured-skills.mjs                 # 聚合脚本（需重写）
└── .github/workflows/
    └── update-featured-skills.yml                # 每日定时运行（保持不变）
```

## 数据源 — 精选仓库列表

从 GitHub Topic 盲目搜索改为直接指定高质量仓库：

| 仓库 | Stars | 说明 |
|------|-------|------|
| `anthropics/skills` | ~96k | Anthropic 官方 Agent Skills |
| `sickn33/antigravity-awesome-skills` | ~24.7k | 1000+ 社区 skills 集合 |
| `K-Dense-AI/claude-scientific-skills` | ~15.3k | 170+ 科学研究 skills |
| `travisvn/awesome-claude-skills` | ~9.1k | 精选 Claude skills 列表 |
| `VoltAgent/awesome-agent-skills` | ~8.4k | 500+ Agent skills |
| `anthropics/knowledge-work-plugins` | ~7.7k | 官方知识工作插件 |
| `alirezarezvani/claude-skills` | ~5.2k | 192+ 社区 skills |

列表定义在脚本顶部 `CURATED_REPOS` 常量中，新增/移除仓库只需编辑此数组。

## 脚本重写逻辑

### 1. 数据采集（仓库元数据）

通过 GitHub Repos API 逐个获取精选仓库的元数据：

```
GET /repos/{owner}/{repo}
```

返回：`full_name`, `description`, `stargazers_count`, `topics[]`, `updated_at`, `html_url`, `default_branch`

认证：**必须**使用环境变量 `GITHUB_TOKEN`，脚本启动时校验。

失败处理：获取失败的仓库跳过（打印警告），不影响其余仓库。

### 2. Skill 检测（仓库内深入扫描）

使用 GitHub Git Trees API 获取仓库目录结构（单次请求，`recursive=1`）：

```
GET /repos/{owner}/{repo}/git/trees/{default_branch}?recursive=1
```

#### Skill 目录扫描规则（与应用端 `installer.rs` 保持一致）

扫描以下基础路径下的子目录：

```
skills/
skills/.curated/
skills/.experimental/
skills/.system/
.claude/skills/
```

以及**根目录的直接子目录**（排除 `skills/`、`.claude/`、`.git/` 等特殊目录）。

#### 新增：`.claude-plugin/plugin.json` 检测

为兼容 `anthropics/knowledge-work-plugins` 等使用插件格式的仓库，增加检测规则：

- 根级子目录包含 `.claude-plugin/plugin.json` 文件 → 视为有效 skill

#### Skill 判定条件

一个目录被视为有效 skill，需满足以下任一条件：
- 目录内存在 `SKILL.md` 文件
- 目录位于 `.claude/skills/` 路径下（即使没有 `SKILL.md`）
- 目录内存在 `.claude-plugin/plugin.json` 文件（插件格式）

#### 单 skill 仓库

如果仓库根目录本身就是一个 skill（根目录有 `SKILL.md`），且没有检测到子目录 skill，则整个仓库视为单 skill，`source_url` 指向仓库根路径。

#### Skill 名称与描述

- **名称**：从 skill 目录名生成（kebab-case → Title Case）
- **描述**：使用仓库的 `description` 字段（同仓库内所有 skill 共享）

> 注：不使用 Contents API 获取 SKILL.md 内容，避免大量 API 调用。目录名 + 仓库 description 已满足展示需求。

### 3. 自动分类

基于仓库 `topics[]` 和 `description` 关键词匹配（分类继承自仓库，同仓库内所有 skill 共享分类）：

| 关键词 | 分类 |
|--------|------|
| browser, automation, playwright, puppeteer | browser-automation |
| security, audit, vulnerability, pentest | security |
| devops, deploy, infra, docker, kubernetes | devops |
| marketing, seo, ads, advertising | marketing |
| database, sql, postgres, mongo | database |
| git, github, pr, code-review | development |
| ai, llm, agent, model | ai-assistant |
| 以上都不匹配 | general |

### 4. 排序与截取

1. 按仓库 `stargazers_count` 降序，同星数按 skill 名称字母序
2. **截取前 300 条**（`MAX_SKILLS = 300`），避免输出过大

### 5. 输出

生成 `featured-skills.json`，**向前兼容现有数据结构**：

```json
{
  "updated_at": "2026-03-19T00:00:00Z",
  "total": 300,
  "categories": ["general", "browser-automation", "security", "devops", ...],
  "skills": [
    {
      "slug": "commit",
      "name": "Conventional Commit",
      "summary": "Generate conventional commit messages...",
      "downloads": 0,
      "stars": 96000,
      "category": "development",
      "tags": ["claude-code-skill", "git", "commit"],
      "source_url": "https://github.com/anthropics/skills/tree/main/skills/commit",
      "updated_at": "2026-03-17T15:10:09Z"
    }
  ]
}
```

#### 向前兼容策略

新格式**必须保留现有字段**，确保后端 `FeaturedSkillRaw` 和前端 `FeaturedSkillDto` 无需任何改动即可解析新数据：

| 字段 | 现有 | 新版 | 兼容处理 |
|------|------|------|----------|
| `slug` | ✅ | ✅ | 不变 |
| `name` | ✅ | ✅ | 不变 |
| `summary` | ✅ | ✅ | 不变 |
| `downloads` | ✅ | ✅ | **保留，固定为 0**（无真实数据源） |
| `stars` | ✅ | ✅ | 填入 GitHub 真实 star 数 |
| `source_url` | ✅ | ✅ | 精确到 skill 目录的路径 |
| `category` | ❌ | ✅ 新增 | 后端 `#[serde(default)]` 自动忽略未知字段 |
| `tags` | ❌ | ✅ 新增 | 同上 |
| `updated_at`（skill 级） | ❌ | ✅ 新增 | 同上 |

**结论**：脚本输出格式升级后，后端和前端代码**零改动**即可正常工作。

#### 字段说明

- `slug`：skill 目录名（单 skill 仓库则为仓库名）
- `name`：基于 skill 目录名生成（kebab-case → Title Case）
- `summary`：仓库 `description`（同仓库内所有 skill 共享）
- `downloads`：固定为 `0`（向前兼容，无真实数据源）
- `stars`：所属仓库的 GitHub star 数
- `category`：基于仓库 topics/description 的自动分类结果
- `tags`：仓库 `topics[]`（最多取 5 个）
- `source_url`：**精确到 skill 目录的 GitHub URL**，格式为 `https://github.com/{owner}/{repo}/tree/{branch}/{skill-path}`；单 skill 仓库则为 `https://github.com/{owner}/{repo}`
- `updated_at`：仓库最后更新时间

## API 用量估算

| 阶段 | API | 请求数 | 说明 |
|------|-----|--------|------|
| 获取仓库元数据 | Repos API | 7 | 每个精选仓库 1 次 |
| 获取目录树 | Git Trees API | 7 | 每个仓库 1 次 |
| **合计** | | **14** | |

对比旧方案（Topic 搜索）的 ~412 次请求，新方案仅需 14 次，**几乎不可能触发速率限制**。

GitHub API 限额（已认证）：5000 次/小时。即使未认证（60 次/小时）也完全够用，但仍建议使用 token 以确保稳定性。

## GitHub Actions 配置

保持现有 `.github/workflows/update-featured-skills.yml` 不变：

```yaml
- name: Fetch featured skills
  run: node scripts/fetch-featured-skills.mjs
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## SkillSyncer 应用端适配

**后端和前端代码无需任何改动**（向前兼容）：

- `FeaturedSkillRaw` / `FeaturedSkillDto` 结构体不变
- 新增的 `category`、`tags`、`updated_at` 等字段被 serde 自动忽略
- 内嵌 fallback（`featured-skills.json`）随脚本运行自动更新

### 后续可选增强（独立需求）

1. 后端 DTO 新增 `category`、`tags` 字段以支持前端展示
2. 探索页面增加按分类筛选
3. 排序改为按星数排序

## 技术要点

- **精选仓库列表**：质量可控，新增仓库只需编辑 `CURATED_REPOS` 数组
- **极低 API 用量**：14 次请求 vs 旧方案 412 次，无速率限制风险
- **零外部依赖**：仅依赖 GitHub API（公开、稳定、有 SLA）
- **Skill 粒度聚合**：每条记录精确到仓库内具体 skill 目录
- **与应用检测逻辑一致**：skill 扫描规则与 `installer.rs` 保持同步
- **新增插件格式支持**：兼容 `.claude-plugin/plugin.json` 结构
- **数量上限**：最多 300 条，按星数排序取 top
- **项目内维护**：脚本、数据、CI 全部在 skills-desktop-app 仓库内

## 变更清单

| 文件 | 操作 |
|------|------|
| `scripts/fetch-featured-skills.mjs` | 重写（精选仓库列表 + Repos API + Trees API） |
| `featured-skills.json` | 内容更新（精选仓库来源，≤300 条） |
| `docs/requirements/skills-aggregation-repo.md` | 更新需求文档 |
| `.github/workflows/update-featured-skills.yml` | **无需改动** |
| 后端代码 | **无需改动**（向前兼容） |
| 前端代码 | **无需改动**（向前兼容） |
