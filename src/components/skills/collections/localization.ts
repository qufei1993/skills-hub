import type { TFunction } from "i18next";
import type { Collection, CollectionIndexItem } from "./types.ts";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function displayFields(
  value: unknown,
  textFields: string[],
  arrayFields: string[] = [],
  objectFields: string[] = [],
): value is Record<string, unknown> {
  if (!isRecord(value)) return false;
  return Object.entries(value).every(([key, field]) => {
    if (textFields.includes(key))
      return typeof field === "string" && !!field.trim();
    if (arrayFields.includes(key))
      return (
        Array.isArray(field) &&
        field.every((item) => typeof item === "string" && !!item.trim())
      );
    return objectFields.includes(key) && isRecord(field);
  });
}

export function validLocalizations(
  value: unknown,
  kind: "index" | "detail",
  skillIds = new Set<string>(),
  evidenceIds = new Set<string>(),
): boolean {
  if (value === undefined) return true;
  if (!isRecord(value) || Object.keys(value).some((key) => key !== "en"))
    return false;
  if (!Object.hasOwn(value, "en")) return true;
  const english = value.en;
  if (kind === "index")
    return displayFields(
      english,
      ["title", "description", "coverAlt", "searchText"],
      ["tags"],
    );
  if (
    !displayFields(
      english,
      ["title", "description", "coverAlt", "overview", "usage"],
      ["tags", "scenarios"],
      ["skills", "provenance"],
    )
  )
    return false;
  if (
    english.skills !== undefined &&
    (!isRecord(english.skills) ||
      !Object.entries(english.skills).every(
        ([id, translation]) =>
          skillIds.has(id) && displayFields(translation, ["summary", "role"]),
      ))
  )
    return false;
  if (english.provenance !== undefined) {
    if (
      !displayFields(english.provenance, ["selectionReason"], [], ["evidence"])
    )
      return false;
    const evidence = english.provenance.evidence;
    if (
      evidence !== undefined &&
      (!isRecord(evidence) ||
        !Object.entries(evidence).every(
          ([id, translation]) =>
            evidenceIds.has(id) &&
            displayFields(translation, ["title", "location", "finding"]),
        ))
    )
      return false;
  }
  return true;
}

export function localizeCollection(
  collection: Collection,
  language: "zh" | "en",
): Collection {
  const english = collection.localizations?.en;
  if (language !== "en" || !english) return collection;
  return {
    ...collection,
    title: english.title ?? collection.title,
    description: english.description ?? collection.description,
    tags: english.tags ?? collection.tags,
    cover: {
      ...collection.cover,
      alt: english.coverAlt ?? collection.cover.alt,
    },
    overview: english.overview ?? collection.overview,
    scenarios: english.scenarios ?? collection.scenarios,
    usage: english.usage ?? collection.usage,
    skills: collection.skills.map((skill) => ({
      ...skill,
      summary: english.skills?.[skill.id]?.summary ?? skill.summary,
      role: english.skills?.[skill.id]?.role ?? skill.role,
    })),
    provenance: {
      ...collection.provenance,
      selectionReason:
        english.provenance?.selectionReason ??
        collection.provenance.selectionReason,
      evidence: collection.provenance.evidence.map((evidence) => {
        const translation = english.provenance?.evidence?.[evidence.id];
        return {
          ...evidence,
          title: translation?.title ?? evidence.title,
          location: translation?.location ?? evidence.location,
          finding: translation?.finding ?? evidence.finding,
        };
      }),
    },
  };
}

export function localizeCollectionIndex(
  item: CollectionIndexItem,
  language: "zh" | "en",
  t: TFunction,
): CollectionIndexItem {
  if (language !== "en") return item;
  const english = item.localizations?.en;
  return {
    ...item,
    title: english?.title ?? item.title,
    description: english?.description ?? item.description,
    tags: english?.tags ?? item.tags,
    cover: { ...item.cover, alt: english?.coverAlt ?? item.cover.alt },
    authorLabel:
      item.authorLabel === "Skills Hub 整理"
        ? t("catalog.editorialAuthor")
        : item.authorLabel.replace(/^(.*) 推荐$/, (_, name: string) =>
            t("catalog.recommendedBy", { name }),
          ),
    licenseLabel: item.licenseLabel?.replaceAll(
      "许可待核对",
      t("catalog.licenseNeedsReview"),
    ),
    searchText:
      english?.searchText && !item.searchText.includes(english.searchText)
        ? `${item.searchText} ${english.searchText}`
        : item.searchText,
  };
}
