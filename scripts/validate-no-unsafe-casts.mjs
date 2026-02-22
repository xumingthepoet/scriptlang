#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const sourceRoot = path.join(root, "src");

const isTsSource = (filePath) => filePath.endsWith(".ts") || filePath.endsWith(".tsx");

const collectFiles = (dir) => {
  const results = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...collectFiles(fullPath));
      continue;
    }
    if (entry.isFile() && isTsSource(fullPath)) {
      results.push(fullPath);
    }
  }
  return results;
};

const findUnsafeCastMatches = (sourceText) => {
  const patterns = [
    { label: "as any", regex: /\bas\s+any\b/g },
    { label: "as unknown as", regex: /\bas\s+unknown\s+as\b/g },
  ];

  const matches = [];
  for (const pattern of patterns) {
    let match = pattern.regex.exec(sourceText);
    while (match) {
      matches.push({
        label: pattern.label,
        index: match.index,
      });
      match = pattern.regex.exec(sourceText);
    }
  }
  return matches.sort((a, b) => a.index - b.index);
};

const indexToLineCol = (sourceText, index) => {
  const prior = sourceText.slice(0, index);
  const lines = prior.split("\n");
  const line = lines.length;
  const col = lines[lines.length - 1].length + 1;
  return { line, col };
};

if (!fs.existsSync(sourceRoot)) {
  console.error("Cast validation failed: src/ directory is missing.");
  process.exit(1);
}

const files = collectFiles(sourceRoot);
const violations = [];

for (const filePath of files) {
  const sourceText = fs.readFileSync(filePath, "utf8");
  const matches = findUnsafeCastMatches(sourceText);
  for (const match of matches) {
    const pos = indexToLineCol(sourceText, match.index);
    violations.push({
      filePath: path.relative(root, filePath),
      line: pos.line,
      col: pos.col,
      label: match.label,
    });
  }
}

if (violations.length > 0) {
  console.error("Unsafe cast validation failed:");
  for (const violation of violations) {
    console.error(
      `- ${violation.filePath}:${violation.line}:${violation.col} uses forbidden pattern "${violation.label}"`
    );
  }
  process.exit(1);
}

console.log("Unsafe cast validation passed.");
