#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const ROOT = process.cwd();
function read(filePath) {
  return fs.readFileSync(path.join(ROOT, filePath), "utf8");
}

function write(filePath, contents) {
  fs.writeFileSync(path.join(ROOT, filePath), contents, "utf8");
}

function replaceJsonStringProp(filePath, propName, newValue) {
  const original = read(filePath);
  const escapedPropName = propName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(`("${escapedPropName}"\\s*:\\s*")([^"]*)(")`);
  const m = original.match(re);
  if (!m) throw new Error(`Cannot find "${propName}" in ${filePath}`);
  const updated = original.replace(re, `$1${newValue}$3`);
  JSON.parse(updated);
  if (updated !== original) write(filePath, updated);
  return { from: m[2], to: newValue, changed: updated !== original };
}

function replaceCargoPackageVersion(filePath, newValue) {
  const original = read(filePath);
  const pkgHeader = original.match(/^\[package\]\s*$/m);
  if (!pkgHeader) throw new Error(`Cannot find [package] section in ${filePath}`);
  const pkgStart = pkgHeader.index ?? 0;
  const afterPkg = original.slice(pkgStart + pkgHeader[0].length);
  const nextSection = afterPkg.match(/^\[[^\]]+\]\s*$/m);
  const pkgEnd = nextSection?.index != null ? pkgStart + pkgHeader[0].length + nextSection.index : original.length;

  const before = original.slice(0, pkgStart);
  const pkgSection = original.slice(pkgStart, pkgEnd);
  const after = original.slice(pkgEnd);

  const re = /^version\s*=\s*"([^"]*)"\s*$/m;
  const m = pkgSection.match(re);
  if (!m) throw new Error(`Cannot find package version in ${filePath}`);
  const updatedSection = pkgSection.replace(re, `version = "${newValue}"`);

  const updated = `${before}${updatedSection}${after}`;
  if (updated !== original) write(filePath, updated);
  return { from: m[1], to: newValue, changed: updated !== original };
}

function getPackageJsonVersion() {
  const pkg = JSON.parse(read("package.json"));
  if (!pkg.version || typeof pkg.version !== "string") {
    throw new Error("package.json missing valid version");
  }
  return pkg.version;
}

function setPackageJsonVersion(newVersion) {
  return replaceJsonStringProp("package.json", "version", newVersion);
}

function syncLockfiles(version) {
  const lock = JSON.parse(read("package-lock.json"));
  lock.version = version;
  lock.packages[""].version = version;
  write("package-lock.json", `${JSON.stringify(lock, null, 2)}\n`);
  const cargo = read("src-tauri/Cargo.lock");
  const pattern = /(\[\[package\]\]\r?\nname = "app"\r?\nversion = ")[^"]+(")/;
  if (!pattern.test(cargo)) throw new Error("src-tauri/Cargo.lock missing app package");
  write("src-tauri/Cargo.lock", cargo.replace(pattern, `$1${version}$2`));
}

function syncFromPackageJson() {
  const version = getPackageJsonVersion();
  const results = [];
  results.push({ file: "package.json", ...(replaceJsonStringProp("package.json", "version", version)) });
  results.push({ file: "src-tauri/tauri.conf.json", ...(replaceJsonStringProp("src-tauri/tauri.conf.json", "version", version)) });
  results.push({ file: "src-tauri/Cargo.toml", ...(replaceCargoPackageVersion("src-tauri/Cargo.toml", version)) });
  syncLockfiles(version);
  return { version, results };
}

function checkInSync() {
  const version = getPackageJsonVersion();
  const mismatches = [];

  const tauriConfVersion = JSON.parse(read("src-tauri/tauri.conf.json")).version;
  if (tauriConfVersion !== version) {
    mismatches.push(`src-tauri/tauri.conf.json version=${tauriConfVersion} (expected ${version})`);
  }

  const cargoToml = read("src-tauri/Cargo.toml");
  const pkgHeader = cargoToml.match(/^\[package\]\s*$/m);
  if (!pkgHeader) throw new Error("src-tauri/Cargo.toml missing [package] section");
  const pkgStart = pkgHeader.index ?? 0;
  const afterPkg = cargoToml.slice(pkgStart + pkgHeader[0].length);
  const nextSection = afterPkg.match(/^\[[^\]]+\]\s*$/m);
  const pkgEnd = nextSection?.index != null ? pkgStart + pkgHeader[0].length + nextSection.index : cargoToml.length;
  const pkgSection = cargoToml.slice(pkgStart, pkgEnd);
  const m = pkgSection.match(/^version\s*=\s*"([^"]*)"\s*$/m);
  if (!m) throw new Error("src-tauri/Cargo.toml missing package version");
  const cargoVersion = m[1];
  if (cargoVersion !== version) {
    mismatches.push(`src-tauri/Cargo.toml version=${cargoVersion} (expected ${version})`);
  }

  const lock = JSON.parse(read("package-lock.json"));
  if (lock.version !== version || lock.packages?.[""]?.version !== version) {
    mismatches.push(`package-lock.json product versions do not match ${version}`);
  }
  const lockedCargoVersion = read("src-tauri/Cargo.lock").match(/\[\[package\]\]\r?\nname = "app"\r?\nversion = "([^"]+)"/)?.[1];
  if (lockedCargoVersion !== version) {
    mismatches.push(`src-tauri/Cargo.lock version=${lockedCargoVersion} (expected ${version})`);
  }

  return { version, mismatches };
}

function usage() {
  console.log("Usage:");
  console.log("  node scripts/version.mjs set <x.y.z>");
  console.log("  node scripts/version.mjs sync");
  console.log("  node scripts/version.mjs check");
}

async function main() {
  const [cmd, arg] = process.argv.slice(2);
  if (!cmd) {
    usage();
    process.exit(1);
  }

  if (cmd === "set") {
    if (!arg) {
      usage();
      process.exit(1);
    }
    setPackageJsonVersion(arg);
    syncFromPackageJson();
    console.log(`Version set to ${arg}`);
    return;
  }

  if (cmd === "sync") {
    const { version, results } = syncFromPackageJson();
    const changedFiles = results.filter((r) => r.changed).map((r) => r.file);
    console.log(`Synced version ${version}${changedFiles.length ? ` (updated: ${changedFiles.join(", ")})` : ""}`);
    return;
  }

  if (cmd === "check") {
    const { version, mismatches } = checkInSync();
    if (mismatches.length) {
      console.error(`Version mismatch (package.json=${version}):`);
      for (const line of mismatches) console.error(`- ${line}`);
      process.exit(1);
    }
    console.log(`Version OK (${version})`);
    return;
  }

  usage();
  process.exit(1);
}

main().catch((err) => {
  console.error(err?.stack || String(err));
  process.exit(1);
});
