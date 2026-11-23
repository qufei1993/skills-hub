import type { Collection, InstallationItem } from "./types.ts";
import type { CollectionManifest } from "../types";

export type InstallConflict =
  { code: "name"; name: string } | { code: "directory"; directory: string };

export interface InstallPlan {
  items: InstallationItem[];
  manifest: CollectionManifest;
  displayNames: (string | undefined)[];
  conflicts: InstallConflict[];
}

function sourceKey(item: InstallationItem): string {
  return `${item.repo.toLowerCase()}\0${item.commit.toLowerCase()}`;
}

function installationKey(item: InstallationItem): string {
  return `${sourceKey(item)}\0${item.directory}`;
}

export function createInstallPlan(
  collection: Collection,
  selectedIds: string[],
): InstallPlan {
  const selected = new Set(selectedIds);
  const installations = new Map<string, InstallationItem>();
  const authorNames = new Map<string, string>();
  for (const skill of collection.skills) {
    if (!selected.has(skill.id)) continue;
    for (const item of skill.installationItems) {
      const key = installationKey(item);
      if (!installations.has(key)) installations.set(key, item);
      const entryPoint = item.directory === "." ? item.entryPoint : `${item.directory}/${item.entryPoint}`;
      if (entryPoint === skill.entry.entryPoint
        && item.repo.toLowerCase() === skill.entry.repo.toLowerCase()
        && item.commit.toLowerCase() === skill.entry.commit.toLowerCase()) {
        authorNames.set(key, skill.name);
      }
    }
  }

  const items = [...installations.values()];
  const conflicts = new Map<string, InstallConflict>();
  const names = new Map<string, string>();
  const directoryVersions = new Map<string, string>();
  const sources: InstallPlan["manifest"]["sources"] = [];
  const sourceIndices = new Map<string, number>();
  const skills: InstallPlan["manifest"]["skills"] = [];

  for (const item of items) {
    const key = installationKey(item);
    const previousName = names.get(item.name);
    if (previousName !== undefined && previousName !== key) {
      conflicts.set(`name\0${item.name}`, { code: "name", name: item.name });
    }
    names.set(item.name, key);

    const directoryKey = `${item.repo.toLowerCase()}\0${item.directory}`;
    const version = item.commit.toLowerCase();
    const previousVersion = directoryVersions.get(directoryKey);
    if (previousVersion !== undefined && previousVersion !== version) {
      conflicts.set(`directory\0${directoryKey}`, {
        code: "directory",
        directory: `${item.repo}/${item.directory}`,
      });
    }
    directoryVersions.set(directoryKey, version);

    const source = sourceKey(item);
    let sourceIndex = sourceIndices.get(source);
    if (sourceIndex === undefined) {
      sourceIndex = sources.length;
      sourceIndices.set(source, sourceIndex);
      sources.push({ repo: item.repo, ref: item.commit });
    }
    skills.push({
      name: item.name,
      path: item.directory,
      source: sourceIndex,
    });
  }

  return {
    items,
    manifest: { v: 1, title: collection.title, sources, skills },
    displayNames: items.map(item => authorNames.get(installationKey(item))),
    conflicts: [...conflicts.values()],
  };
}

export function repositoryList(collection: Collection): string[] {
  const repositories = new Map<string, string>();
  for (const skill of collection.skills) {
    for (const item of skill.installationItems) {
      const key = item.repo.toLowerCase();
      if (!repositories.has(key)) repositories.set(key, item.repo);
    }
  }
  return [...repositories.values()];
}
