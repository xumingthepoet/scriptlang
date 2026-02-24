import fs from "node:fs";
import path from "node:path";

import {
  PLAYER_COMPILER_VERSION,
  chooseAndContinue,
  resumeScenario,
  startScenario,
} from "../src/cli/core/engine-runner.js";
import { loadSourceByScriptsDir } from "../src/cli/core/source-loader.js";
import type { ChoiceItem, SnapshotV1 } from "../src/core/types.js";

interface ChoiceTraversalOptions {
  maxChoiceSteps: number;
  maxRuntimeMs: number;
  maxPaths: number;
  verbose: boolean;
}

interface TraversalTarget {
  label: string;
  scriptsDir: string;
}

interface TraversalSummary {
  label: string;
  scriptsDir: string;
  completedPaths: number;
  transitions: number;
  maxDepth: number;
  elapsedMs: number;
}

interface TraceStep {
  choiceIndex: number;
  choiceId: string;
  choiceText: string;
}

interface ParsedArgs {
  scriptsDir?: string;
  examplesRoot?: string;
  options: ChoiceTraversalOptions;
}

class TraversalError extends Error {
  readonly code: string;
  readonly target: string;
  readonly trace: TraceStep[];

  constructor(code: string, message: string, target: string, trace: TraceStep[] = []) {
    super(message);
    this.name = "TraversalError";
    this.code = code;
    this.target = target;
    this.trace = trace;
  }
}

const DEFAULT_OPTIONS: ChoiceTraversalOptions = {
  maxChoiceSteps: 100,
  maxRuntimeMs: 30_000,
  maxPaths: 20_000,
  verbose: false,
};

const usage = [
  "choice-traversal",
  "  tsx scripts/choice-traversal.ts [--scripts-dir <path>] [--examples-root <path>]",
  "                                 [--max-choice-steps <n>] [--max-runtime-ms <ms>]",
  "                                 [--max-paths <n>] [--verbose]",
  "",
  "defaults:",
  "  --examples-root examples/scripts",
  "  --max-choice-steps 100",
  "  --max-runtime-ms 30000",
  "  --max-paths 20000",
].join("\n");

const normalizeIntArg = (name: string, raw: string): number => {
  const parsed = Number.parseInt(raw, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`Invalid ${name} value: ${raw}`);
  }
  return parsed;
};

const parseArgs = (argv: string[]): ParsedArgs => {
  const parsed: ParsedArgs = {
    options: { ...DEFAULT_OPTIONS },
    examplesRoot: path.resolve("examples", "scripts"),
  };

  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === "--help" || token === "-h") {
      process.stdout.write(`${usage}\n`);
      process.exit(0);
    }
    if (token === "--scripts-dir") {
      parsed.scriptsDir = path.resolve(argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (token === "--examples-root") {
      parsed.examplesRoot = path.resolve(argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (token === "--max-choice-steps") {
      parsed.options.maxChoiceSteps = normalizeIntArg(token, argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (token === "--max-runtime-ms") {
      parsed.options.maxRuntimeMs = normalizeIntArg(token, argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (token === "--max-paths") {
      parsed.options.maxPaths = normalizeIntArg(token, argv[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (token === "--verbose") {
      parsed.options.verbose = true;
      continue;
    }
    throw new Error(`Unknown argument: ${token}`);
  }

  return parsed;
};

const formatTrace = (trace: TraceStep[]): string => {
  if (trace.length === 0) {
    return "<root>";
  }
  return trace
    .map(
      (step, index) =>
        `${index}:${step.choiceIndex}[${step.choiceId}](${step.choiceText.replace(/\s+/g, " ").trim()})`
    )
    .join(" -> ");
};

const collectTargets = (args: ParsedArgs): TraversalTarget[] => {
  if (args.scriptsDir) {
    return [{ label: path.basename(args.scriptsDir), scriptsDir: args.scriptsDir }];
  }

  const root = args.examplesRoot as string;
  if (!fs.existsSync(root) || !fs.statSync(root).isDirectory()) {
    throw new Error(`Examples root is not a directory: ${root}`);
  }

  return fs
    .readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => ({
      label: entry.name,
      scriptsDir: path.join(root, entry.name),
    }))
    .filter((target) => fs.existsSync(path.join(target.scriptsDir, "main.script.xml")))
    .sort((a, b) => a.label.localeCompare(b.label));
};

const assertWithinRuntime = (
  target: TraversalTarget,
  startedAt: number,
  options: ChoiceTraversalOptions,
  trace: TraceStep[]
): void => {
  const elapsed = Date.now() - startedAt;
  if (elapsed > options.maxRuntimeMs) {
    throw new TraversalError(
      "TRAVERSE_TIMEOUT",
      `Traversal exceeded max runtime ${options.maxRuntimeMs}ms (elapsed=${elapsed}ms).`,
      target.label,
      trace
    );
  }
};

const assertWithinPathLimit = (
  target: TraversalTarget,
  completedPaths: number,
  options: ChoiceTraversalOptions,
  trace: TraceStep[]
): void => {
  if (completedPaths > options.maxPaths) {
    throw new TraversalError(
      "TRAVERSE_MAX_PATHS",
      `Traversal exceeded max paths ${options.maxPaths}.`,
      target.label,
      trace
    );
  }
};

const traverseScenario = (
  target: TraversalTarget,
  options: ChoiceTraversalOptions
): TraversalSummary => {
  const scenario = loadSourceByScriptsDir(target.scriptsDir);
  const started = startScenario(scenario, PLAYER_COMPILER_VERSION);
  const startedAt = Date.now();

  let completedPaths = 0;
  let transitions = 0;
  let maxDepth = 0;

  const walk = (snapshot: SnapshotV1, depth: number, trace: TraceStep[]): void => {
    assertWithinRuntime(target, startedAt, options, trace);

    const resumed = resumeScenario(scenario, snapshot, PLAYER_COMPILER_VERSION);
    if (resumed.boundary.event !== "CHOICES") {
      throw new TraversalError(
        "TRAVERSE_BOUNDARY_INVALID",
        "Expected CHOICES boundary while traversing snapshot state.",
        target.label,
        trace
      );
    }

    const choices = resumed.boundary.choices;
    if (choices.length === 0) {
      throw new TraversalError(
        "TRAVERSE_EMPTY_CHOICES",
        "Encountered empty choices list during traversal.",
        target.label,
        trace
      );
    }

    for (const item of choices) {
      const nextTrace: TraceStep[] = [
        ...trace,
        {
          choiceIndex: item.index,
          choiceId: item.id,
          choiceText: item.text,
        },
      ];
      const nextDepth = depth + 1;
      maxDepth = Math.max(maxDepth, nextDepth);

      if (nextDepth > options.maxChoiceSteps) {
        throw new TraversalError(
          "TRAVERSE_MAX_STEPS",
          `Traversal exceeded max choice steps ${options.maxChoiceSteps}.`,
          target.label,
          nextTrace
        );
      }

      const branch = resumeScenario(scenario, snapshot, PLAYER_COMPILER_VERSION);
      if (branch.boundary.event !== "CHOICES") {
        throw new TraversalError(
          "TRAVERSE_BRANCH_INVALID",
          "Expected CHOICES boundary before applying branch choice.",
          target.label,
          nextTrace
        );
      }

      const boundary = chooseAndContinue(branch.engine, item.index);
      transitions += 1;
      assertWithinRuntime(target, startedAt, options, nextTrace);

      if (boundary.event === "END") {
        completedPaths += 1;
        assertWithinPathLimit(target, completedPaths, options, nextTrace);
        continue;
      }

      const nextSnapshot = branch.engine.snapshot();
      walk(nextSnapshot, nextDepth, nextTrace);
    }
  };

  if (started.boundary.event === "END") {
    completedPaths = 1;
  } else {
    walk(started.engine.snapshot(), 0, []);
  }

  return {
    label: target.label,
    scriptsDir: target.scriptsDir,
    completedPaths,
    transitions,
    maxDepth,
    elapsedMs: Date.now() - startedAt,
  };
};

export const runChoiceTraversal = (
  argv: string[]
): { summaries: TraversalSummary[]; failed: boolean } => {
  const args = parseArgs(argv);
  const targets = collectTargets(args);
  const summaries: TraversalSummary[] = [];

  if (targets.length === 0) {
    throw new Error("No traversal targets found.");
  }

  for (const target of targets) {
    try {
      const summary = traverseScenario(target, args.options);
      summaries.push(summary);
      process.stdout.write(
        `[PASS] ${summary.label} paths=${summary.completedPaths} transitions=${summary.transitions} maxDepth=${summary.maxDepth} elapsed=${summary.elapsedMs}ms\n`
      );
    } catch (error) {
      if (error instanceof TraversalError) {
        process.stderr.write(
          `[FAIL] ${error.target} code=${error.code} msg=${error.message}\ntrace: ${formatTrace(error.trace)}\n`
        );
        return { summaries, failed: true };
      }
      const message = error instanceof Error ? error.message : "Unknown traversal error.";
      process.stderr.write(`[FAIL] ${target.label} code=TRAVERSE_CRASH msg=${message}\n`);
      return { summaries, failed: true };
    }
  }

  if (args.options.verbose) {
    const totals = summaries.reduce(
      (acc, summary) => {
        acc.paths += summary.completedPaths;
        acc.transitions += summary.transitions;
        acc.maxDepth = Math.max(acc.maxDepth, summary.maxDepth);
        acc.elapsedMs += summary.elapsedMs;
        return acc;
      },
      { paths: 0, transitions: 0, maxDepth: 0, elapsedMs: 0 }
    );

    process.stdout.write(
      `[SUMMARY] scenarios=${summaries.length} paths=${totals.paths} transitions=${totals.transitions} maxDepth=${totals.maxDepth} elapsed=${totals.elapsedMs}ms\n`
    );
  }

  return { summaries, failed: false };
};

const main = (): void => {
  try {
    const result = runChoiceTraversal(process.argv.slice(2));
    process.exitCode = result.failed ? 1 : 0;
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown traversal setup error.";
    process.stderr.write(`${message}\n`);
    process.stderr.write(`${usage}\n`);
    process.exitCode = 1;
  }
};

main();
