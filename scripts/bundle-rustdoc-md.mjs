#!/usr/bin/env node
/**
 * Bundles cargo-doc-md output into a single Markdown API reference.
 *
 * Usage:
 *   node scripts/bundle-rustdoc-md.mjs <title> <target-subdir> <crate-folder> <out.md>
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(__dirname, "..");

function walkMdFiles(dir) {
  const out = [];
  if (!fs.existsSync(dir)) return out;
  for (const name of fs.readdirSync(dir).sort()) {
    if (name.startsWith(".")) continue;
    const candidate = path.join(dir, name);
    const stat = fs.statSync(candidate);
    if (stat.isDirectory()) out.push(...walkMdFiles(candidate));
    else if (name.endsWith(".md")) out.push(candidate);
  }
  return out;
}

function readText(filePath) {
  return fs.readFileSync(filePath, "utf8").trimEnd();
}

const [, , title, targetSubdir, crateFolder, outArg] = process.argv;
if (!title || !targetSubdir || !crateFolder || !outArg) {
  console.error(
    "Usage: node scripts/bundle-rustdoc-md.mjs <title> <target-subdir> <crate-folder> <out.md>",
  );
  process.exit(1);
}

const inRoot = path.join(root, "target", targetSubdir);
const moduleRoot = path.join(inRoot, crateFolder);
const outPath = path.resolve(root, outArg);

if (!fs.existsSync(moduleRoot)) {
  console.error(`Missing ${path.relative(root, moduleRoot)}; run cargo doc-md first.`);
  process.exit(1);
}

const filesByRel = new Map(
  walkMdFiles(moduleRoot).map((filePath) => [
    path.relative(moduleRoot, filePath).split(path.sep).join("/"),
    filePath,
  ]),
);
const ordered = [];
for (const preferred of ["index.md", `${crateFolder}.md`]) {
  if (filesByRel.has(preferred)) {
    ordered.push([preferred, filesByRel.get(preferred)]);
    filesByRel.delete(preferred);
  }
}
for (const rel of [...filesByRel.keys()].sort((a, b) => a.localeCompare(b))) {
  ordered.push([rel, filesByRel.get(rel)]);
}

const pieces = [
  `# ${title} - generated API (Markdown)`,
  "",
  "> Do not edit by hand; regenerate with `npm run docs:fel-core`.",
  "",
  "Bundled from cargo-doc-md output. Nested module paths are preserved in headings.",
  "",
  "---",
  "",
];

const topIndex = path.join(inRoot, "index.md");
if (fs.existsSync(topIndex)) {
  pieces.push("## doc-md index", "", readText(topIndex), "", "---", "");
}

for (const [label, filePath] of ordered) {
  pieces.push(`## Source: ${crateFolder}/${label}`, "", readText(filePath), "", "---", "");
}

fs.mkdirSync(path.dirname(outPath), { recursive: true });
fs.writeFileSync(outPath, `${pieces.join("\n").trimEnd()}\n`, "utf8");
console.log(`Wrote ${path.relative(root, outPath)}`);
