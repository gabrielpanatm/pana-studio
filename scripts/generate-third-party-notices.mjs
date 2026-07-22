#!/usr/bin/env node

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(scriptDirectory, "..");
const outputPath = join(
  projectRoot,
  "src-tauri/resources/licenses/THIRD_PARTY_LICENSES.txt",
);
const cargoManifestPath = join(projectRoot, "src-tauri/Cargo.toml");
const checkOnly = process.argv.includes("--check");
const legalFilePattern = /^(licen[cs]e|copying|notice|copyright)(?:$|[._-])/i;
const maximumLegalFileBytes = 2 * 1024 * 1024;

function normalizedText(value) {
  return value.replace(/\r\n/g, "\n").trim();
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function packageNameFromLockPath(lockPath) {
  const marker = "node_modules/";
  const index = lockPath.lastIndexOf(marker);
  return index === -1 ? lockPath : lockPath.slice(index + marker.length);
}

function legalFilesInDirectory(directory) {
  if (!existsSync(directory)) return [];

  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile() && legalFilePattern.test(entry.name))
    .map((entry) => join(directory, entry.name));
}

function readableLegalFile(path) {
  if (!existsSync(path)) return null;
  const stats = statSync(path);
  if (!stats.isFile() || stats.size > maximumLegalFileBytes) return null;
  const text = normalizedText(readFileSync(path, "utf8"));
  return text ? { name: path.split("/").at(-1), text } : null;
}

function npmPackages() {
  const lock = readJson(join(projectRoot, "package-lock.json"));

  return Object.entries(lock.packages ?? {})
    .filter(([lockPath]) => lockPath.startsWith("node_modules/"))
    .map(([lockPath, entry]) => {
      const packageDirectory = join(projectRoot, lockPath);
      const manifestPath = join(packageDirectory, "package.json");
      const manifest = existsSync(manifestPath) ? readJson(manifestPath) : {};
      const name = manifest.name ?? packageNameFromLockPath(lockPath);
      const version = entry.version ?? manifest.version ?? "unknown";
      const license = entry.license ?? manifest.license ?? "NOT DECLARED";
      const files = legalFilesInDirectory(packageDirectory);

      return {
        ecosystem: "npm",
        id: `npm:${name}@${version}`,
        name,
        version,
        license,
        source: manifest.repository?.url ?? manifest.homepage ?? entry.resolved ?? "",
        files,
      };
    });
}

function cargoMetadata() {
  const result = spawnSync(
    "cargo",
    [
      "metadata",
      "--locked",
      "--filter-platform",
      "x86_64-unknown-linux-gnu",
      "--format-version=1",
      "--manifest-path",
      cargoManifestPath,
    ],
    {
      cwd: projectRoot,
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
    },
  );

  if (result.status !== 0) {
    process.stderr.write(result.stderr || result.stdout);
    throw new Error("cargo metadata nu a putut genera graful de dependențe.");
  }

  return JSON.parse(result.stdout);
}

function cargoLegalFiles(pkg) {
  const manifestDirectory = dirname(pkg.manifest_path);
  const candidates = new Set(legalFilesInDirectory(manifestDirectory));

  if (pkg.license_file) {
    const explicitPath = resolve(manifestDirectory, pkg.license_file);
    if (existsSync(explicitPath)) candidates.add(explicitPath);
  }

  if ((pkg.source ?? "").startsWith("git+")) {
    let current = manifestDirectory;
    for (let depth = 0; depth < 5; depth += 1) {
      for (const path of legalFilesInDirectory(current)) candidates.add(path);
      const parent = dirname(current);
      if (parent === current) break;
      current = parent;
    }
  }

  return [...candidates];
}

function cargoPackages() {
  const metadata = cargoMetadata();
  const ownManifest = resolve(cargoManifestPath);

  return metadata.packages
    .filter((pkg) => resolve(pkg.manifest_path) !== ownManifest)
    .map((pkg) => {
      const isZola = (pkg.source ?? "").includes("github.com/getzola/zola");
      return {
        ecosystem: "Cargo",
        id: `cargo:${pkg.name}@${pkg.version}`,
        name: pkg.name,
        version: pkg.version,
        license:
          pkg.license ??
          (isZola
            ? "MIT / EUPL-1.2 (conform istoricului fișierelor upstream)"
            : "NOT DECLARED"),
        source: pkg.repository ?? pkg.homepage ?? pkg.source ?? "",
        files: cargoLegalFiles(pkg),
      };
    });
}

function uniquePackages(packages) {
  const byIdentity = new Map();
  for (const pkg of packages) {
    const key = `${pkg.ecosystem}\0${pkg.name}\0${pkg.version}\0${pkg.source}`;
    if (!byIdentity.has(key)) byIdentity.set(key, pkg);
  }

  return [...byIdentity.values()].sort((left, right) =>
    left.id.localeCompare(right.id, "en"),
  );
}

function legalDocuments(packages) {
  const byHash = new Map();
  const packagesWithoutText = [];

  for (const pkg of packages) {
    let found = false;
    for (const path of pkg.files) {
      const document = readableLegalFile(path);
      if (!document) continue;
      found = true;
      const hash = createHash("sha256").update(document.text).digest("hex");
      const existing = byHash.get(hash);
      if (existing) {
        existing.packages.add(pkg.id);
        existing.names.add(document.name);
      } else {
        byHash.set(hash, {
          hash,
          text: document.text,
          packages: new Set([pkg.id]),
          names: new Set([document.name]),
        });
      }
    }
    if (!found) packagesWithoutText.push(pkg);
  }

  return {
    documents: [...byHash.values()].sort((left, right) =>
      [...left.packages][0].localeCompare([...right.packages][0], "en"),
    ),
    packagesWithoutText,
  };
}

function renderInventory(title, packages) {
  const lines = [title, "-".repeat(title.length)];
  for (const pkg of packages) {
    const source = pkg.source ? ` | ${pkg.source}` : "";
    lines.push(`${pkg.name} ${pkg.version} | ${pkg.license}${source}`);
  }
  return lines.join("\n");
}

function renderOutput(packages, documents, packagesWithoutText) {
  const npm = packages.filter((pkg) => pkg.ecosystem === "npm");
  const cargo = packages.filter((pkg) => pkg.ecosystem === "Cargo");
  const sections = [
    "PANA STUDIO — THIRD-PARTY DEPENDENCY LICENCES",
    "================================================",
    "",
    "Generated by scripts/generate-third-party-notices.mjs.",
    "Do not edit this file manually.",
    "",
    "The inventory is derived from package-lock.json and Cargo.lock for the",
    "Linux x86-64 target. It is intentionally conservative and can include",
    "build or development dependencies that are not present in the final binary.",
    "Project-specific notices for the embedded Zola engine and Anime.js are documented in",
    "THIRD_PARTY_NOTICES.md at the repository root and in adjacent licence files.",
    "",
    renderInventory("NPM PACKAGES", npm),
    "",
    renderInventory("CARGO PACKAGES", cargo),
    "",
  ];

  if (packagesWithoutText.length > 0) {
    sections.push(
      "PACKAGES WITHOUT A DISTRIBUTED LICENCE/NOTICE FILE",
      "--------------------------------------------------",
      "The declared SPDX expression remains listed in the inventory above.",
      ...packagesWithoutText.map((pkg) => `${pkg.id} | ${pkg.license}`),
      "",
    );
  }

  sections.push("LICENCE AND NOTICE TEXTS", "========================", "");

  for (const document of documents) {
    const packageList = [...document.packages].sort().join(", ");
    const fileNames = [...document.names].sort().join(", ");
    sections.push(
      `Applies to: ${packageList}`,
      `Source file name(s): ${fileNames}`,
      `Content SHA-256: ${document.hash}`,
      "-".repeat(72),
      document.text,
      "",
      "=".repeat(72),
      "",
    );
  }

  return `${sections.join("\n").trimEnd()}\n`;
}

const packages = uniquePackages([...npmPackages(), ...cargoPackages()]);
const { documents, packagesWithoutText } = legalDocuments(packages);
const output = renderOutput(packages, documents, packagesWithoutText);

if (checkOnly) {
  if (!existsSync(outputPath) || readFileSync(outputPath, "utf8") !== output) {
    process.stderr.write(
      "Inventarul licențelor nu este actualizat. Rulează `npm run licenses:generate`.\n",
    );
    process.exit(1);
  }
  process.stdout.write(
    `Inventarul licențelor este actualizat (${packages.length} pachete, ${documents.length} texte unice).\n`,
  );
} else {
  writeFileSync(outputPath, output, "utf8");
  process.stdout.write(
    `Am generat ${relative(projectRoot, outputPath)} (${packages.length} pachete, ${documents.length} texte unice).\n`,
  );
}
