const assert = require("node:assert/strict");
const { test } = require("node:test");
const path = require("node:path");
const { pathToFileURL } = require("node:url");

const repository = "qufei1993/skills-hub";
const api = `https://api.github.com/repos/${repository}/releases`;
const now = "2026-10-01T06:00:00.000Z";
const sourceCommit = "a".repeat(40);
const repositoryApi = `https://api.github.com/repos/${repository}`;
let implementation;
async function method(name) {
  if (!implementation) {
    try {
      implementation = await import(
        pathToFileURL(path.join(__dirname, "collect-release-data.mjs"))
      );
    } catch (error) {
      if (error.code !== "ERR_MODULE_NOT_FOUND") throw error;
      implementation = {};
    }
  }
  assert.equal(
    typeof implementation[name],
    "function",
    `missing release synchronization behavior: ${name}`,
  );
  return implementation[name];
}

function asset(version, name) {
  return {
    name,
    size: 123456,
    browser_download_url: `https://github.com/${repository}/releases/download/${version}/${encodeURIComponent(name)}`,
  };
}
function release(version = "v0.10.1", changes = {}) {
  return {
    tag_name: version,
    name: `Skills Hub ${version}`,
    draft: false,
    prerelease: false,
    published_at: "2026-09-13T03:02:17Z",
    html_url: `https://github.com/${repository}/releases/tag/${version}`,
    body: "## 中文\n\n### 修复\n- **同步修复**：保留本地修改。\n\n## English\n\n### Fixed\n- Preserve local changes.",
    assets: [asset(version, `Skills-Hub-${version}-macOS-aarch64.dmg`)],
    ...changes,
  };
}
function respond(value, options = {}) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json", ...options.headers },
    ...options,
  });
}
function source(releases, latest = releases[0], overrides = {}) {
  const changelogs = {
    zh: releases
      .map(
        (item) =>
          `## [${item.tag_name.slice(1)}]\n\n### 修复\n- **同步修复**：保留本地修改。`,
      )
      .join("\n\n"),
    en: releases
      .map(
        (item) =>
          `## [${item.tag_name.slice(1)}]\n\n### Fixed\n- Preserve local changes.`,
      )
      .join("\n\n"),
    ...overrides,
  };
  return async (url) => {
    if (url === `${api}?per_page=100`) return respond(releases);
    if (url === `${api}/latest`) return respond(latest);
    if (url === `${repositoryApi}/commits/main`)
      return respond({ sha: sourceCommit });
    for (const [language, file] of [
      ["en", "CHANGELOG.md"],
      ["zh", "docs/CHANGELOG.zh.md"],
    ]) {
      if (url === `${repositoryApi}/contents/${file}?ref=${sourceCommit}`)
        return respond({
          type: "file",
          path: file,
          encoding: "base64",
          content: Buffer.from(changelogs[language]).toString("base64"),
        });
    }
    throw new Error(`Unexpected public API URL: ${url}`);
  };
}
test("Chinese release notes omit the other language and installation boilerplate", async () => {
  const select = await method("selectReleaseNotes");
  const body =
    "## 中文\n\n### 下载安装\n| 系统 | 安装包 |\n| macOS | DMG |\n\n### 修复\n- **同步修复**：保留本地修改。\n\n**Windows 提示：** 安装帮助。\n\n**macOS 提示：** 安装帮助。\n\n## English\n\n### Fixed\n- Preserve local changes.";
  assert.deepEqual(select(body, "v0.10.1"), {
    markdown: "### 修复\n- **同步修复**：保留本地修改。",
    language: "zh",
    summary: "同步修复：保留本地修改。",
  });
});

test("legacy English notes remain available and are marked English", async () => {
  const select = await method("selectReleaseNotes");
  assert.deepEqual(select("### Added\n- Added **local import**.", "v0.1.0"), {
    markdown: "### Added\n- Added **local import**.",
    language: "en",
    summary: "Added local import.",
  });
});

test("long English summaries end at a complete sentence and mark omitted text", async () => {
  const select = await method("selectReleaseNotes");
  const firstSentence =
    "Windows command-window flashing: Automatic-update progress refreshes now read runtime data without querying the operating-system scheduler every five seconds.";
  const selected = select(
    `- ${firstSentence} Windows scheduler and system Git child processes also start without a console window, preventing repeated command-window flashes.`,
    "v0.10.1",
  );
  assert.equal(selected.summary, `${firstSentence}…`);
  assert.ok(Array.from(selected.summary).length <= 160);
});

test("long English summaries without a sentence boundary preserve complete words", async () => {
  const select = await method("selectReleaseNotes");
  const text = "Preserve local changes and synchronize installed skills "
    .repeat(5)
    .trim();
  const selected = select(`- ${text}`, "v0.10.1");
  assert.ok(selected.summary.endsWith("…"));
  assert.ok(text.startsWith(selected.summary.slice(0, -1)));
  assert.match(text.slice(selected.summary.length - 1), /^\s/);
  assert.ok(Array.from(selected.summary).length <= 160);
});

test("long Chinese summaries mark omitted text without splitting Unicode characters", async () => {
  const select = await method("selectReleaseNotes");
  const text = "保留本地修改并同步已安装技能🚀".repeat(20);
  const selected = select(`## 中文\n- ${text}`, "v0.10.1");
  assert.ok(selected.summary.endsWith("…"));
  assert.ok(text.startsWith(selected.summary.slice(0, -1)));
  assert.ok(Array.from(selected.summary).length <= 160);
  assert.doesNotMatch(selected.summary, /\p{Surrogate}/u);
});

test("short summaries retain their original English and Chinese wording", async () => {
  const select = await method("selectReleaseNotes");
  for (const text of ["Preserve local changes.", "保留本地修改。🚀"]) {
    assert.equal(select(`- ${text}`, "v0.10.1").summary, text);
  }
});

test("language headings and download-like headings inside code do not split notes", async () => {
  const select = await method("selectReleaseNotes");
  const selected = select(
    "## 中文\n- 中文说明\n\n```md\n## English\n### Downloads\nkeep me\n```\n\n## English\n- English description",
    "v0.10.1",
  );
  assert.equal(selected.language, "zh");
  assert.match(selected.markdown, /keep me/);
  assert.doesNotMatch(selected.markdown, /English description/);
});

test("relative Markdown links and reference links resolve inside the released tag", async () => {
  const select = await method("selectReleaseNotes");
  const selected = select(
    '- Read [guide](docs/guide.md#setup), [readme](../README.md), ![shot](/images/app.png), and [external](https://example.com/a).\n\n[guide]: ./docs/guide.md "Guide"',
    "v0.9.0",
  );
  assert.match(
    selected.markdown,
    /https:\/\/github.com\/qufei1993\/skills-hub\/blob\/v0\.9\.0\/docs\/guide\.md#setup/,
  );
  assert.match(
    selected.markdown,
    /https:\/\/github.com\/qufei1993\/skills-hub\/blob\/v0\.9\.0\/README\.md/,
  );
  assert.match(
    selected.markdown,
    /https:\/\/github.com\/qufei1993\/skills-hub\/blob\/v0\.9\.0\/images\/app\.png/,
  );
  assert.match(selected.markdown, /\[external\]\(https:\/\/example.com\/a\)/);
  assert.match(
    selected.markdown,
    /\[guide\]: https:\/\/github.com\/qufei1993\/skills-hub\/blob\/v0\.9\.0\/docs\/guide\.md "Guide"/,
  );
});

test("encoded parent segments and Windows separators cannot escape the released tag", async () => {
  const select = await method("selectReleaseNotes");
  const selected = select(
    "- [readme](%2e%2e/README.md) and [guide](docs%2Fguide%20one.md).\n- [windows](..\\README.md)",
    "v0.9.0",
  );
  const destinations = [...selected.markdown.matchAll(/\]\(([^)]+)\)/g)].map(
    (match) => new URL(match[1]).pathname,
  );
  assert.deepEqual(destinations, [
    "/qufei1993/skills-hub/blob/v0.9.0/README.md",
    "/qufei1993/skills-hub/blob/v0.9.0/docs/guide%20one.md",
    "/qufei1993/skills-hub/blob/v0.9.0/README.md",
  ]);
});

test("a notes-less Chinese segment falls back to the actual English changes", async () => {
  const select = await method("selectReleaseNotes");
  const selected = select(
    "## 中文\n\n### 下载安装\n| macOS | DMG |\n\n## English\n### Fixed\n- Fixed import.",
    "v0.10.1",
  );
  assert.equal(selected.language, "en");
  assert.equal(selected.markdown, "### Fixed\n- Fixed import.");
});

test("pagination includes older releases and excludes drafts and prereleases", async () => {
  const collect = await method("collectReleaseSnapshot");
  const next = `${api}?per_page=100&page=2`;
  const fetchImpl = async (url) => {
    if (url === `${api}?per_page=100`)
      return respond(
        [
          release(),
          release("v0.11.0", { draft: true }),
          release("v0.12.0", { prerelease: true }),
        ],
        { headers: { Link: `<${next}>; rel="next"` } },
      );
    if (url === next)
      return respond([
        release("v0.9.0", {
          published_at: "2026-08-23T02:21:44Z",
          body: "### Added\n- Legacy English.",
        }),
      ]);
    if (url === `${api}/latest`) return respond(release());
    return source([release(), release("v0.9.0")])(url);
  };
  const { index, notes } = await collect({
    fetchImpl,
    now: () => new Date(now),
  });
  assert.equal(index.schemaVersion, 1);
  assert.equal(index.repository, repository);
  assert.equal(index.syncedAt, now);
  assert.equal(index.latest, "v0.10.1");
  assert.deepEqual(
    index.releases.map((entry) => entry.version),
    ["v0.10.1", "v0.9.0"],
  );
  assert.equal(index.sourceCommit, sourceCommit);
  assert.deepEqual(index.releases[0].notes, {
    zh: {
      path: "/changelog/zh/v0.10.1.md",
      summary: "同步修复：保留本地修改。",
    },
    en: {
      path: "/changelog/en/v0.10.1.md",
      summary: "Preserve local changes.",
    },
  });
  assert.deepEqual(Object.keys(notes.zh).sort(), ["v0.10.1", "v0.9.0"]);
  assert.deepEqual(Object.keys(notes.en).sort(), ["v0.10.1", "v0.9.0"]);
  assert.equal(Object.hasOwn(index.releases[0], "notesLanguage"), false);
});

test("existing bilingual changelogs supply older Chinese chapters and exclude unpublished versions", async () => {
  const collect = await method("collectReleaseSnapshot");
  const oldest = release("v0.1.0", {
    body: "### Added\n- English release body only.",
  });
  const { index, notes } = await collect({
    fetchImpl: source([oldest], oldest, {
      zh: "# 更新日志\n\n## [Unreleased]\n- 尚未发布\n\n## [0.11.0]\n- 草稿内容\n\n## [0.1.0] - 2026-01-24\n\n### 新增\n- 初次发布。",
      en: "## [0.11.0]\n- Draft\n\n## [0.1.0] - 2026-01-25\n\n### Added\n- First release.",
    }),
  });
  assert.equal(index.releases[0].publishedAt, "2026-09-13T03:02:17.000Z");
  assert.equal(notes.zh["v0.1.0"], "### 新增\n- 初次发布。");
  assert.equal(notes.en["v0.1.0"], "### Added\n- First release.");
  assert.equal(notes.zh["v0.11.0"], undefined);
  assert.doesNotMatch(notes.zh["v0.1.0"], /草稿|尚未发布/);
});

test("a missing chapter is marked unavailable without relabeling another language", async () => {
  const collect = await method("collectReleaseSnapshot");
  const item = release();
  const { index, notes } = await collect({
    fetchImpl: source([item], item, { zh: "# 更新日志\n## [0.9.0]\n- 老版本" }),
  });
  assert.equal(index.releases[0].notes.zh, null);
  assert.equal(notes.zh["v0.10.1"], undefined);
  assert.equal(index.releases[0].notes.en.path, "/changelog/en/v0.10.1.md");
  await assert.rejects(
    collect({
      fetchImpl: source([item], item, { zh: "# 更新日志", en: "# Changelog" }),
    }),
    /missing|no changelog/i,
  );
});

test("Chinese relative links use the docs source directory inside the release tag", async () => {
  const collect = await method("collectReleaseSnapshot");
  const item = release();
  const { notes } = await collect({
    fetchImpl: source([item], item, {
      zh: "## [0.10.1]\n- [guide](guide.md), [readme](../README.md), ![shot](/images/app.png).",
    }),
  });
  assert.match(notes.zh["v0.10.1"], /blob\/v0\.10\.1\/docs\/guide\.md/);
  assert.match(notes.zh["v0.10.1"], /blob\/v0\.10\.1\/README\.md/);
  assert.match(notes.zh["v0.10.1"], /blob\/v0\.10\.1\/images\/app\.png/);
});

test("GitHub latest selects the download version even when another release was published later", async () => {
  const collect = await method("collectReleaseSnapshot");
  const official = release("v0.10.1", { published_at: "2026-09-01T00:00:00Z" });
  const newerDate = release("v0.9.0", { published_at: "2026-09-30T00:00:00Z" });
  const { index } = await collect({
    fetchImpl: source([newerDate, official], official),
  });
  assert.equal(index.latest, "v0.10.1");
});

test("installers map canonical and legacy architectures while signatures, CLI and missing platforms stay absent", async () => {
  const collect = await method("collectReleaseSnapshot");
  const item = release("v0.10.1", {
    assets: [
      asset("v0.10.1", "Skills-Hub-v0.10.1-macOS-arm64.dmg"),
      asset("v0.10.1", "Skills Hub_0.10.1_x86_64.dmg"),
      asset("v0.10.1", "Skills.Hub_0.10.1_x64-setup.exe"),
      asset("v0.10.1", "Skills-Hub-v0.10.1-Windows-aarch64.exe"),
      asset("v0.10.1", "Skills-Hub-v0.10.1-Windows-x64.exe.sig"),
      asset("v0.10.1", "skillshub-cli-windows-x64.exe"),
      asset("v0.10.1", "Skills-Hub-v0.10.1-macOS-aarch64.tar.gz"),
    ],
  });
  const { index } = await collect({ fetchImpl: source([item]) });
  assert.deepEqual(
    index.releases[0].installers.map(({ system, architecture, format }) => [
      system,
      architecture,
      format,
    ]),
    [
      ["macos", "arm64", "DMG"],
      ["macos", "x64", "DMG"],
      ["windows", "x64", "EXE"],
      ["windows", "arm64", "EXE"],
    ],
  );
  assert.equal(
    index.releases[0].installers[1].url,
    "https://github.com/qufei1993/skills-hub/releases/download/v0.10.1/Skills%20Hub_0.10.1_x86_64.dmg",
  );
  const noWindows = release("v0.1.0");
  const oldest = await collect({ fetchImpl: source([noWindows]) });
  assert.deepEqual(
    oldest.index.releases[0].installers.map(({ system }) => system),
    ["macos"],
  );
});

test("empty release lists, missing official latest and duplicate versions abort collection", async () => {
  const collect = await method("collectReleaseSnapshot");
  await assert.rejects(
    collect({ fetchImpl: source([], release()) }),
    /empty|no public/i,
  );
  await assert.rejects(
    collect({ fetchImpl: source([release()], release("v0.12.0")) }),
    /latest/i,
  );
  await assert.rejects(
    collect({ fetchImpl: source([release(), release()]) }),
    /duplicate/i,
  );
});

test("unsafe versions and unrelated release or attachment URLs cannot enter a snapshot", async () => {
  const collect = await method("collectReleaseSnapshot");
  for (const changes of [
    { tag_name: "../../outside" },
    { html_url: "https://evil.example/releases/tag/v0.10.1" },
    {
      assets: [
        {
          ...asset("v0.10.1", "Skills-Hub-v0.10.1-Windows-x64.exe"),
          browser_download_url: "https://evil.example/app.exe",
        },
      ],
    },
    {
      assets: [
        { ...asset("v0.10.1", "Skills-Hub-v0.10.1-Windows-x64.exe"), size: -1 },
      ],
    },
  ]) {
    const item = release("v0.10.1", changes);
    await assert.rejects(
      collect({ fetchImpl: source([item]) }),
      /invalid|unsafe/i,
    );
  }
});

test("external pagination and repeated pagination links abort without requesting untrusted URLs", async () => {
  const collect = await method("collectReleaseSnapshot");
  for (const next of [
    "https://evil.example/releases?page=2",
    `${api}?per_page=100`,
  ]) {
    const fetchImpl = async (url) => {
      if (url !== `${api}?per_page=100`)
        throw new Error("Untrusted request was made");
      return respond([release()], {
        headers: { Link: `<${next}>; rel="next"` },
      });
    };
    await assert.rejects(collect({ fetchImpl }), /pagination|unsafe/i);
  }
});

test("HTTP, malformed responses and timeouts abort the complete snapshot", async () => {
  const collect = await method("collectReleaseSnapshot");
  await assert.rejects(
    collect({
      fetchImpl: async () => new Response("rate limited", { status: 403 }),
    }),
    /403/,
  );
  await assert.rejects(
    collect({ fetchImpl: async () => respond({ message: "not releases" }) }),
    /invalid/i,
  );
  await assert.rejects(
    collect({
      fetchImpl: async () => new Response("not json", { status: 200 }),
    }),
    /JSON|json/i,
  );
  await assert.rejects(
    collect({ fetchImpl: async () => new Promise(() => {}), timeoutMs: 15 }),
    /timeout|timed out/i,
  );
});

test('publication reads notes from the latest stable release tag, never main', async () => {
  const collect = await method('collectReleaseSnapshot');
  const item = release(); const fallback = source([item], item); const requests=[];
  const { index } = await collect({ sourceRef:'latest', fetchImpl:async url => {
    requests.push(url);
    return fallback(url === `${repositoryApi}/commits/${item.tag_name}` ? `${repositoryApi}/commits/main` : url);
  }});
  assert.equal(index.latest,item.tag_name);
  assert.ok(requests.includes(`${repositoryApi}/commits/${item.tag_name}`));
  assert.ok(!requests.includes(`${repositoryApi}/commits/main`));
});
