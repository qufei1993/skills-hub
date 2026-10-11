import { validLocalizations } from "./localization.ts";
import type {
  Collection,
  CollectionCover,
  CollectionIndexItem,
} from "./types.ts";

const categories = new Set([
  "development",
  "design",
  "product",
  "writing",
  "marketing",
  "research",
  "data",
  "productivity",
  "media",
  "other",
]);
const uuid =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function strings(value: unknown): value is string[] {
  return (
    Array.isArray(value) && value.every((item) => typeof item === "string")
  );
}

function hasStrings(value: Record<string, unknown>, fields: string[]): boolean {
  return fields.every((field) => typeof value[field] === "string");
}

function optionalString(
  value: Record<string, unknown>,
  field: string,
): boolean {
  return value[field] === undefined || typeof value[field] === "string";
}

function validSource(
  value: unknown,
): value is Record<string, unknown> & { repo: string; commit: string } {
  return (
    isRecord(value) &&
    typeof value.repo === "string" &&
    /^[a-z0-9][a-z0-9-]*\/[a-z0-9_.-]+$/i.test(value.repo) &&
    ![".", ".."].includes(value.repo.split("/")[1]) &&
    typeof value.commit === "string" &&
    /^[a-f0-9]{40}$/i.test(value.commit)
  );
}

function validRelativePath(value: unknown, allowRoot = false): boolean {
  if (typeof value !== "string" || !value.trim()) return false;
  if (allowRoot && value === ".") return true;
  return (
    !value.startsWith("/") &&
    !/^[a-z]:/i.test(value) &&
    !value.includes("\\") &&
    value.split("/").every((part) => part && part !== "." && part !== "..")
  );
}

function validCover(value: unknown): value is CollectionCover {
  if (!isRecord(value) || typeof value.alt !== "string") return false;
  if (value.kind === "concept")
    return ["design", "engineering", "marketing", "video", "neutral"].includes(
      String(value.preset),
    );
  return (
    value.kind === "source-image" &&
    hasStrings(value, ["url", "sourceUrl"]) &&
    optionalString(value, "background") &&
    optionalString(value, "position") &&
    ["verified", "needs-review"].includes(String(value.rightsStatus))
  );
}

function validIdentity(value: Record<string, unknown>): boolean {
  return (
    value.schemaVersion === 1 &&
    typeof value.id === "string" &&
    uuid.test(value.id) &&
    typeof value.slug === "string" &&
    /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(value.slug) &&
    Number.isSafeInteger(value.revision) &&
    Number(value.revision) > 0
  );
}

export function validateIndex(value: unknown): CollectionIndexItem[] {
  if (
    !isRecord(value) ||
    value.schemaVersion !== 1 ||
    !Array.isArray(value.collections)
  )
    throw new Error("Invalid collection index");
  const ids = new Set<string>();
  const slugs = new Set<string>();
  for (const item of value.collections) {
    if (
      !isRecord(item) ||
      !validIdentity(item) ||
      !hasStrings(item, [
        "title",
        "description",
        "authorLabel",
        "searchText",
      ]) ||
      !optionalString(item, "licenseLabel") ||
      !validLocalizations(item.localizations, "index") ||
      !categories.has(String(item.category)) ||
      !strings(item.tags) ||
      !validCover(item.cover) ||
      !Number.isSafeInteger(item.itemCount) ||
      Number(item.itemCount) < 1 ||
      !Number.isSafeInteger(item.installationCount) ||
      Number(item.installationCount) < 1
    )
      throw new Error("Invalid collection index item");
    const id = String(item.id).toLowerCase();
    const slug = String(item.slug);
    if (ids.has(id) || slugs.has(slug))
      throw new Error("Duplicate collection index identity");
    ids.add(id);
    slugs.add(slug);
  }
  return value.collections as CollectionIndexItem[];
}

function validProvenance(value: unknown): value is Collection["provenance"] {
  if (
    !isRecord(value) ||
    !Array.isArray(value.evidence) ||
    value.evidence.length === 0 ||
    !value.evidence.every(
      (evidence) =>
        isRecord(evidence) &&
        hasStrings(evidence, [
          "id",
          "url",
          "title",
          "publisher",
          "checkedAt",
          "location",
          "finding",
        ]) &&
        ["repository", "article", "post"].includes(String(evidence.kind)) &&
        strings(evidence.supportsItemIds) &&
        optionalString(evidence, "publishedAt"),
    )
  )
    return false;
  for (const person of [value.author, value.recommender]) {
    if (
      person !== undefined &&
      (!isRecord(person) || !hasStrings(person, ["name", "url"]))
    )
      return false;
  }
  if (value.repositoryScope !== undefined) {
    const scope = value.repositoryScope;
    if (
      !isRecord(scope) ||
      !hasStrings(scope, ["repo", "commit"]) ||
      !strings(scope.directories)
    )
      return false;
  }
  return optionalString(value, "selectionReason");
}

function validVerification(value: unknown): boolean {
  return (
    isRecord(value) &&
    ["verified", "needs-review"].includes(String(value.source)) &&
    ["verified", "needs-review"].includes(String(value.files)) &&
    ["not-tested", "passed", "failed"].includes(String(value.installation)) &&
    ["not-tested", "passed", "failed"].includes(String(value.workflow)) &&
    typeof value.checkedAt === "string" &&
    strings(value.notes)
  );
}

export function validateDetail(value: unknown, preview = false): Collection {
  if (
    !isRecord(value) ||
    !validIdentity(value) ||
    value.status !== "active" ||
    !hasStrings(value, ["title", "description", "overview", "usage"]) ||
    !categories.has(String(value.category)) ||
    !strings(value.tags) ||
    !strings(value.scenarios) ||
    !validCover(value.cover) ||
    ![
      "author-repository",
      "external-recommendation",
      "editorial-selection",
    ].includes(String(value.sourceKind)) ||
    !validProvenance(value.provenance) ||
    !validVerification(value.verification) ||
    !isRecord(value.review) ||
    !(
      value.review.status === "approved" ||
      (preview && value.review.status === "pending")
    ) ||
    !strings(value.review.notes) ||
    !Array.isArray(value.skills) ||
    value.skills.length === 0
  )
    throw new Error("Invalid collection detail");
  const ids = new Set<string>();
  for (const skill of value.skills) {
    if (
      !isRecord(skill) ||
      !hasStrings(skill, ["id", "name", "summary", "resourceNotes"]) ||
      !optionalString(skill, "role") ||
      !optionalString(skill, "originalDescription") ||
      !["verified", "needs-review"].includes(String(skill.resourceCheck)) ||
      !["skill", "bundle"].includes(String(skill.installationMode)) ||
      !validSource(skill.entry) ||
      typeof skill.entry.entryPoint !== "string" ||
      !validRelativePath(skill.entry.entryPoint) ||
      !/(?:^|\/)SKILL\.md$/.test(skill.entry.entryPoint) ||
      !Array.isArray(skill.installationItems) ||
      skill.installationItems.length === 0 ||
      (skill.installationMode === "skill" &&
        skill.installationItems.length !== 1) ||
      (skill.installationMode === "bundle" &&
        skill.installationItems.length < 2) ||
      !skill.installationItems.every(
        (item) =>
          isRecord(item) &&
          validSource(item) &&
          hasStrings(item, ["name", "directory"]) &&
          validRelativePath(item.directory, true) &&
          item.entryPoint === "SKILL.md" &&
          isRecord(item.license) &&
          typeof item.license.identifier === "string",
      )
    )
      throw new Error("Invalid collection Skill");
    const entry = skill.entry;
    if (
      !skill.installationItems.some(
        (item) =>
          validSource(item) &&
          item.repo.toLowerCase() === entry.repo.toLowerCase() &&
          item.commit.toLowerCase() === entry.commit.toLowerCase() &&
          (item.directory === "."
            ? item.entryPoint
            : `${item.directory}/${item.entryPoint}`) === entry.entryPoint,
      )
    )
      throw new Error("Collection entry is absent from its installation items");
    const id = String(skill.id);
    if (ids.has(id)) throw new Error("Duplicate collection Skill identity");
    ids.add(id);
  }
  if (
    !validLocalizations(
      value.localizations,
      "detail",
      ids,
      new Set(
        value.provenance.evidence.map(
          (evidence: { id: string }) => evidence.id,
        ),
      ),
    )
  )
    throw new Error("Invalid collection localizations");
  return value as unknown as Collection;
}

