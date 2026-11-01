import path from "node:path";

const repository = "qufei1993/skills-hub";
const apiUrl = `https://api.github.com/repos/${repository}/releases`;
const versionPattern = /^v\d+\.\d+\.\d+$/;

function validateVersion(version) {
  if (typeof version !== "string" || !versionPattern.test(version))
    throw new Error("Invalid release version");
  return version;
}

function validateGitHubUrl(value, expectedPath) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error("Invalid GitHub release URL");
  }
  if (
    url.protocol !== "https:" ||
    url.hostname !== "github.com" ||
    url.port ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    decodeURIComponent(url.pathname) !== expectedPath
  )
    throw new Error("Unsafe GitHub release URL");
  return url.href;
}

function linesOutsideFences(markdown) {
  let fence = null;
  return markdown.split(/\r?\n/).map((line) => {
    const delimiter = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
    const outside = fence === null;
    if (delimiter) {
      if (!fence)
        fence = { marker: delimiter[1][0], length: delimiter[1].length };
      else if (
        delimiter[1][0] === fence.marker &&
        delimiter[1].length >= fence.length &&
        !delimiter[2].trim()
      )
        fence = null;
    }
    return { line, outside: outside && !delimiter };
  });
}

function rewriteDestination(destination, version, sourceFile) {
  const angle = destination.startsWith("<") && destination.endsWith(">");
  const value = angle ? destination.slice(1, -1) : destination;
  if (/^(?:[a-z][a-z\d+.-]*:|\/\/|#)/i.test(value)) return destination;
  const split = value.search(/[?#]/);
  const file = split < 0 ? value : value.slice(0, split);
  const suffix = split < 0 ? "" : value.slice(split);
  let decoded;
  try {
    decoded = decodeURIComponent(file).replaceAll("\\", "/");
  } catch {
    throw new Error("Invalid relative Markdown URL");
  }
  const sourceDirectory = sourceFile ? path.posix.dirname(sourceFile) : "";
  const normalized = path.posix
    .normalize(
      decoded.startsWith("/") ? decoded : `/${sourceDirectory}/${decoded}`,
    )
    .slice(1)
    .split("/")
    .map(encodeURIComponent)
    .join("/");
  const absolute = `https://github.com/${repository}/blob/${version}/${normalized}${suffix}`;
  return angle ? `<${absolute}>` : absolute;
}

function truncateSummary(text, maximumLength = 160) {
  const characters = Array.from(text);
  if (characters.length <= maximumLength) return text;
  const prefix = characters.slice(0, maximumLength - 1).join("");
  const sentenceEnds = Array.from(prefix.matchAll(/[。！？]|[.!?](?=\s|$)/gu));
  const sentenceEnd = sentenceEnds.at(-1);
  if (sentenceEnd) {
    const sentence = prefix.slice(0, sentenceEnd.index + sentenceEnd[0].length);
    if (Array.from(sentence).length >= maximumLength / 2) return `${sentence}…`;
  }
  const wordCharacter = /[\p{Script=Latin}\p{Number}'’_-]/u;
  const completePrefix = wordCharacter.test(characters[maximumLength - 1])
    ? prefix.replace(/[\p{Script=Latin}\p{Number}'’_-]+$/u, "")
    : prefix;
  return `${completePrefix.trimEnd()}…`;
}

export function selectReleaseNotes(body, version, sourceFile = "") {
  validateVersion(version);
  if (typeof body !== "string" || !body.trim() || body.length > 1_000_000)
    throw new Error("Invalid or empty release notes");
  const lines = linesOutsideFences(body);
  const sections = [];
  lines.forEach(({ line, outside }, index) => {
    const heading =
      outside && line.match(/^ {0,3}##[\t ]+(中文|English)[\t ]*$/i);
    if (heading)
      sections.push({
        start: index + 1,
        language: heading[1] === "中文" ? "zh" : "en",
      });
  });
  let selected = lines;
  let language = "en";
  for (const candidate of ["zh", "en"]) {
    const sectionIndex = sections.findIndex(
      (section) => section.language === candidate,
    );
    if (sectionIndex < 0) continue;
    const section = sections[sectionIndex];
    const end = sections[sectionIndex + 1]?.start - 1 || lines.length;
    const candidateLines = lines.slice(section.start, end);
    if (!candidateLines.some(({ line }) => line.trim())) continue;
    selected = candidateLines;
    language = candidate;
    break;
  }
  let downloadLevel = null;
  const kept = [];
  for (const { line, outside } of selected) {
    const heading = outside && line.match(/^ {0,3}(#{1,3})[\t ]+(.*)$/);
    if (heading && downloadLevel !== null && heading[1].length <= downloadLevel)
      downloadLevel = null;
    if (heading && /^(Downloads|下载安装)[\t ]*$/i.test(heading[2])) {
      downloadLevel = heading[1].length;
      continue;
    }
    if (downloadLevel !== null) continue;
    if (
      outside &&
      /^\*\*(?:Windows|macOS)[\t ]+(?:提示|Note)[:：]\*\*/i.test(line)
    )
      continue;
    if (outside) {
      kept.push(
        line
          .replace(
            /\]\((<[^>]*>|[^\s)]+)([^)]*)\)/g,
            (_match, destination, rest) =>
              `](${rewriteDestination(destination, version, sourceFile)}${rest})`,
          )
          .replace(
            /^( {0,3}\[[^\]]+\]:[\t ]*)(<[^>]*>|[^\s]+)(.*)$/,
            (_match, prefix, destination, rest) =>
              `${prefix}${rewriteDestination(destination, version, sourceFile)}${rest}`,
          ),
      );
    } else kept.push(line);
  }
  const markdown = kept.join("\n").trim();
  if (!markdown) {
    const englishIndex = sections.findIndex(
      (section) => section.language === "en",
    );
    if (language === "zh" && englishIndex >= 0) {
      const english = sections[englishIndex];
      const end = sections[englishIndex + 1]?.start - 1 || lines.length;
      return selectReleaseNotes(
        lines
          .slice(english.start, end)
          .map(({ line }) => line)
          .join("\n"),
        version,
        sourceFile,
      );
    }
    throw new Error("Empty release notes after selecting language");
  }
  const summaryLine = linesOutsideFences(markdown).find(
    ({ line, outside }) =>
      outside && line.trim() && !/^\s*(?:#|\[.*\]:|<|\|)/.test(line),
  );
  const summary = truncateSummary(
    (summaryLine?.line ?? "")
      .replace(/^\s*(?:[-*+]\s+|\d+[.)]\s+)/, "")
      .replace(/!?\[([^\]]+)\]\([^)]*\)/g, "$1")
      .replace(/[*_`]/g, "")
      .trim(),
  );
  return { markdown, language, summary };
}

function getInstallers(assets, version) {
  if (!Array.isArray(assets)) throw new Error("Invalid release assets");
  const installers = [];
  const keys = new Set();
  for (const asset of assets) {
    if (!asset || typeof asset.name !== "string")
      throw new Error("Invalid release asset name");
    const name = asset.name;
    if (
      !/^skills[-_. ]hub(?:[-_. ]|$)/i.test(name) ||
      !/\.(dmg|exe)$/i.test(name)
    )
      continue;
    if (/[\\/\x00-\x1f\x7f]/.test(name))
      throw new Error("Invalid release installer name");
    const arm = /(?:^|[-_. ()])(?:aarch64|arm64)(?:[-_. ()]|$)/i.test(name);
    const intel =
      /(?:^|[-_. ()])(?:x86[_-]64|x64|amd64|intel)(?:[-_. ()]|$)/i.test(name);
    if (arm === intel) continue;
    const system = /\.dmg$/i.test(name) ? "macos" : "windows";
    const architecture = arm ? "arm64" : "x64";
    const key = `${system}:${architecture}`;
    if (keys.has(key))
      throw new Error("Duplicate release installer architecture");
    keys.add(key);
    if (!Number.isSafeInteger(asset.size) || asset.size <= 0)
      throw new Error("Invalid release installer size");
    installers.push({
      system,
      architecture,
      format: system === "macos" ? "DMG" : "EXE",
      name,
      url: validateGitHubUrl(
        asset.browser_download_url,
        `/${repository}/releases/download/${version}/${name}`,
      ),
      size: asset.size,
    });
  }
  return installers;
}

async function requestJson(url, fetchImpl, timeoutMs) {
  const controller = new AbortController();
  let timer;
  try {
    return await Promise.race([
      (async () => {
        const response = await fetchImpl(url, {
          headers: {
            Accept: "application/vnd.github+json",
            "User-Agent": "Skills-Hub-Website",
          },
          signal: controller.signal,
        });
        if (!response.ok) throw new Error(`GitHub HTTP ${response.status}`);
        return {
          data: await response.json(),
          link: response.headers.get("link"),
        };
      })(),
      new Promise((_resolve, reject) => {
        timer = setTimeout(() => {
          reject(new Error("GitHub request timed out"));
          controller.abort();
        }, timeoutMs);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

function nextPage(link) {
  if (!link) return null;
  const next = link.split(",").find((part) => /;\s*rel="next"/.test(part));
  if (!next) return null;
  const destination = next.match(/<([^>]+)>/)?.[1];
  let url;
  try {
    url = new URL(destination);
  } catch {
    throw new Error("Invalid pagination URL");
  }
  if (
    url.origin !== "https://api.github.com" ||
    url.pathname !== `/repos/${repository}/releases` ||
    url.username ||
    url.password ||
    url.hash ||
    [...url.searchParams].some(
      ([key, value]) =>
        !["page", "per_page"].includes(key) || !/^[1-9]\d*$/.test(value),
    )
  )
    throw new Error("Unsafe pagination URL");
  return url.href;
}

function extractChangelogSection(changelog, version) {
  const target = version.slice(1).replaceAll(".", "\\.");
  const heading = new RegExp(
    `^##\\s+(?:\\[v?${target}\\]|v?${target})(?:\\s*-.*)?$`,
  );
  const lines = linesOutsideFences(changelog);
  const start = lines.findIndex(
    ({ line, outside }) => outside && heading.test(line),
  );
  if (start < 0) return null;
  const end = lines.findIndex(
    ({ line, outside }, index) =>
      index > start && outside && /^##\s+/.test(line),
  );
  const section = lines
    .slice(start + 1, end < 0 ? lines.length : end)
    .map(({ line }) => line)
    .join("\n")
    .trim();
  return section || null;
}

async function readChangelogs(fetchImpl, timeoutMs, sourceRef) {
  const repositoryApi = `https://api.github.com/repos/${repository}`;
  const { data: commit } = await requestJson(
    `${repositoryApi}/commits/${encodeURIComponent(sourceRef)}`,
    fetchImpl,
    timeoutMs,
  );
  if (
    !commit ||
    typeof commit.sha !== "string" ||
    !/^[a-f0-9]{40}$/i.test(commit.sha)
  )
    throw new Error("Invalid changelog source commit");
  const changelogs = {};
  for (const [language, file] of [
    ["zh", "docs/CHANGELOG.zh.md"],
    ["en", "CHANGELOG.md"],
  ]) {
    const { data } = await requestJson(
      `${repositoryApi}/contents/${file}?ref=${commit.sha}`,
      fetchImpl,
      timeoutMs,
    );
    if (
      !data ||
      data.type !== "file" ||
      data.path !== file ||
      data.encoding !== "base64" ||
      typeof data.content !== "string" ||
      !/^[A-Za-z0-9+/=\r\n]+$/.test(data.content) ||
      data.content.length > 2_000_000
    )
      throw new Error(`Invalid ${language} changelog source`);
    changelogs[language] = {
      file,
      text: Buffer.from(data.content, "base64").toString("utf8"),
    };
  }
  return { sourceCommit: commit.sha.toLowerCase(), changelogs };
}

export async function collectReleaseSnapshot({
  fetchImpl = fetch,
  timeoutMs = 12000,
  now = () => new Date(),
  sourceRef = "main",
} = {}) {
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0)
    throw new Error("Invalid request timeout");
  const releases = [];
  const pages = new Set();
  let url = `${apiUrl}?per_page=100`;
  while (url) {
    if (pages.has(url) || pages.size >= 100)
      throw new Error("Repeated or excessive pagination");
    pages.add(url);
    const result = await requestJson(url, fetchImpl, timeoutMs);
    if (!Array.isArray(result.data))
      throw new Error("Invalid GitHub release list");
    for (const release of result.data) {
      if (
        !release ||
        typeof release.draft !== "boolean" ||
        typeof release.prerelease !== "boolean"
      )
        throw new Error("Invalid release publication state");
      if (!release.draft && !release.prerelease) releases.push(release);
    }
    url = nextPage(result.link);
  }
  if (!releases.length)
    throw new Error("No public releases: empty release list");
  const { data: latest } = await requestJson(
    `${apiUrl}/latest`,
    fetchImpl,
    timeoutMs,
  );
  if (!latest || latest.draft !== false || latest.prerelease !== false)
    throw new Error("Invalid official latest release");
  const latestVersion = validateVersion(latest.tag_name);
  const { sourceCommit, changelogs } = await readChangelogs(
    fetchImpl,
    timeoutMs,
    sourceRef === "latest" ? latestVersion : sourceRef,
  );
  const versions = new Set();
  const notes = { zh: {}, en: {} };
  const entries = releases
    .map((release) => {
      const version = validateVersion(release.tag_name);
      if (versions.has(version)) throw new Error("Duplicate release version");
      versions.add(version);
      if (
        typeof release.published_at !== "string" ||
        !Number.isFinite(Date.parse(release.published_at))
      )
        throw new Error("Invalid release publication date");
      if (typeof release.name !== "string" || !release.name.trim())
        throw new Error("Invalid release name");
      const localizedNotes = { zh: null, en: null };
      for (const language of ["zh", "en"]) {
        const source = changelogs[language];
        const section = extractChangelogSection(source.text, version);
        if (!section) continue;
        const selected = selectReleaseNotes(section, version, source.file);
        notes[language][version] = selected.markdown;
        localizedNotes[language] = {
          path: `/changelog/${language}/${version}.md`,
          summary: selected.summary,
        };
      }
      if (!localizedNotes.zh && !localizedNotes.en)
        throw new Error(
          `No changelog entries for release ${version}: both languages missing`,
        );
      return {
        version,
        name: release.name,
        publishedAt: new Date(release.published_at).toISOString(),
        notes: localizedNotes,
        htmlUrl: validateGitHubUrl(
          release.html_url,
          `/${repository}/releases/tag/${version}`,
        ),
        installers: getInstallers(release.assets, version),
      };
    })
    .sort(
      (left, right) =>
        Date.parse(right.publishedAt) - Date.parse(left.publishedAt),
    );
  if (!versions.has(latestVersion))
    throw new Error(
      "Official latest release is missing from the complete index",
    );
  return {
    index: {
      schemaVersion: 1,
      repository,
      syncedAt: now().toISOString(),
      latest: latestVersion,
      sourceCommit,
      releases: entries,
    },
    notes,
  };
}
