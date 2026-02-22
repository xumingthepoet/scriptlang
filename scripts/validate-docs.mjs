#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = process.cwd();

const requiredPaths = [
  "AGENTS.md",
  "README.md",
  "ARCHITECTURE.md",
  "PLANS.md",
  "docs/README.md",
  "docs/design-docs/core-beliefs.md",
  "docs/product-specs/index.md",
  "docs/exec-plans/README.md",
  "docs/exec-plans/tech-debt-tracker.md",
  "docs/references/agent-first-engineering.md",
  "docs/QUALITY_SCORE.md",
  "docs/RELIABILITY.md",
  "docs/SECURITY.md",
];

const errors = [];

for (const rel of requiredPaths) {
  const full = path.join(root, rel);
  if (!fs.existsSync(full)) {
    errors.push(`Missing required file: ${rel}`);
  }
}

const readIfExists = (rel) => {
  const full = path.join(root, rel);
  return fs.existsSync(full) ? fs.readFileSync(full, "utf8") : "";
};

const agents = readIfExists("AGENTS.md");
if (!agents.includes("Required Workflow")) {
  errors.push("AGENTS.md must include a 'Required Workflow' section.");
}

const architecture = readIfExists("ARCHITECTURE.md");
if (!architecture.includes("Layered System")) {
  errors.push("ARCHITECTURE.md must include a 'Layered System' section.");
}

const refs = readIfExists("docs/references/agent-first-engineering.md");
if (!refs.includes("harness-engineering")) {
  errors.push("Reference doc must include the harness engineering link.");
}

if (errors.length > 0) {
  console.error("Documentation validation failed:");
  for (const e of errors) {
    console.error(`- ${e}`);
  }
  process.exit(1);
}

console.log("Documentation validation passed.");
