#!/usr/bin/env node
/**
 * Bundles cargo-doc-md output into a single Markdown API reference.
 *
 * Usage (from the checkout containing the rustdoc output):
 *   node scripts/bundle-rustdoc-md.mjs <title> <target-subdir> <crate-folder> <out.md>
 */
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();

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
  return fs.readFileSync(filePath, "utf8").trimEnd()
    .replace(/^\*\*Traits:\*\* (.+)$/gm,
      (_, list) => `**Traits:** ${list.split(", ").sort().join(", ")}`)
    .replace(/^(\*\*Trait Implementations:\*\*\n\n)((?:- \*\*[^\n]+(?:\n  [^\n]*)*\n?)+)/gm,
      (_, heading, list) => {
        // Keep each trait with its indented methods.
        const blocks = list.trimEnd().split(/\n(?=- \*\*)/).sort();
        return heading + blocks.join("\n") + (list.endsWith("\n") ? "\n" : "");
      });
}

const [, , title, targetSubdir, crateFolder, outArg] = process.argv;
if (!title || !targetSubdir || !crateFolder || !outArg) {
  console.error(
    "Usage: node scripts/bundle-rustdoc-md.mjs <title> <target-subdir> <crate-folder> <out.md>",
  );
  process.exit(1);
}

const inRoot = path.resolve(root, "target", targetSubdir);
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
for (const rel of [...filesByRel.keys()].sort()) {
  ordered.push([rel, filesByRel.get(rel)]);
}

const pieces = [
  `# ${title} — generated API (Markdown)`,
  "",
  "> Do not edit by hand; regenerate via npm script / cargo doc-md + this bundler.",
  "",
  "Bundled from [cargo-doc-md](https://github.com/Crazytieguy/cargo-doc-md). Nested module paths are preserved in headings. Relative links may not resolve; search by heading.",
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
