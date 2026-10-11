export type CollectionCategory =
  | "development"
  | "design"
  | "product"
  | "writing"
  | "marketing"
  | "research"
  | "data"
  | "productivity"
  | "media"
  | "other";

export type CollectionCover =
  | {
      kind: "source-image";
      url: string;
      sourceUrl: string;
      alt: string;
      rightsStatus: "verified" | "needs-review";
      rightsEvidenceUrl?: string;
      width?: number;
      height?: number;
      background?: string;
      position?: string;
    }
  | {
      kind: "concept";
      preset: "design" | "engineering" | "marketing" | "video" | "neutral";
      alt: string;
    };

export interface InstallationItem {
  name: string;
  repo: string;
  commit: string;
  directory: string;
  entryPoint: "SKILL.md";
  license: { identifier: string; evidenceUrl?: string };
}

export interface CollectionSkill {
  id: string;
  name: string;
  summary: string;
  role?: string;
  authors?: { name: string; evidenceUrl: string }[];
  originalDescription?: string;
  entry: { repo: string; commit: string; entryPoint: string };
  installationMode: "skill" | "bundle";
  installationItems: InstallationItem[];
  resourceCheck: "verified" | "needs-review";
  resourceNotes: string;
}

export interface CollectionIndexLocalization {
  title?: string;
  description?: string;
  tags?: string[];
  coverAlt?: string;
  searchText?: string;
}

export interface CollectionLocalization extends Omit<
  CollectionIndexLocalization,
  "searchText"
> {
  overview?: string;
  scenarios?: string[];
  usage?: string;
  skills?: Record<string, { summary?: string; role?: string }>;
  provenance?: {
    selectionReason?: string;
    evidence?: Record<
      string,
      { title?: string; location?: string; finding?: string }
    >;
  };
}

export interface CollectionIndexItem {
  schemaVersion: 1;
  id: string;
  slug: string;
  title: string;
  description: string;
  category: CollectionCategory;
  tags: string[];
  cover: CollectionCover;
  revision: number;
  authorLabel: string;
  licenseLabel?: string;
  itemCount: number;
  installationCount: number;
  searchText: string;
  localizations?: { en?: CollectionIndexLocalization };
}

export interface CollectionTestEvidence {
  testedAt: string;
  versions: { repo: string; commit: string }[];
  itemIds: string[];
  resultRef: string;
  environment: string;
}

export interface Collection {
  schemaVersion: 1;
  id: string;
  slug: string;
  revision: number;
  sortOrder?: number;
  createdAt: string;
  updatedAt: string;
  sourceKeys: string[];
  status: "active" | "archived";
  submission?:
    | { operation: "create"; baseRevision: null }
    | { operation: "update"; baseRevision: number };
  sourceKind:
    "author-repository" | "external-recommendation" | "editorial-selection";
  title: string;
  description: string;
  category: CollectionCategory;
  tags: string[];
  overview: string;
  scenarios: string[];
  usage: string;
  localizations?: { en?: CollectionLocalization };
  provenance: {
    author?: { name: string; url: string };
    recommender?: { name: string; url: string };
    repositoryScope?: {
      repo: string;
      commit: string;
      directories: string[];
      coverage: "all-valid-skills";
    };
    selectionReason?: string;
    evidence: {
      id: string;
      kind: "repository" | "article" | "post";
      url: string;
      title: string;
      publisher: string;
      publishedAt?: string;
      checkedAt: string;
      location: string;
      finding: string;
      supportsItemIds: string[];
    }[];
  };
  cover: CollectionCover;
  skills: CollectionSkill[];
  verification: {
    source: "verified" | "needs-review";
    files: "verified" | "needs-review";
    installation: "not-tested" | "passed" | "failed";
    workflow: "not-tested" | "passed" | "failed";
    checkedAt: string;
    notes: string[];
    installationEvidence?: CollectionTestEvidence;
    workflowEvidence?: CollectionTestEvidence;
  };
  review: {
    status: "pending" | "approved" | "rejected";
    notes: string[];
    reviewer?: string;
    reviewedAt?: string;
  };
}
