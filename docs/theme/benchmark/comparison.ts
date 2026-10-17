import { readFileSync } from "node:fs";

export type Pair = { nuten: number; nufmt: number };

export type Locked = { rev: string; date: string };

export type Excluded = { formatter: string; reason: string; files: string[] };

export type Comparison = {
  measured_at: string;
  runs: number;
  versions: {
    nuten: string | null;
    nufmt: Locked | null;
    nushell: Locked | null;
    corpus: Record<string, Locked>;
  };
  environment: { os: string; cpu: string; cpus: number };
  files: number;
  excluded: Excluded[];
  timing: {
    files: { file: string; memory_peak_resident: Pair; instructions: Pair }[];
  };
  directories: {
    name: string;
    files: number;
    all_cpus: { time_wall_clock: Pair; memory_peak_resident: Pair };
    one_cpu: { time_wall_clock: Pair } | null;
  }[];
  sizes: Pair;
};

const CHECKED_REASONS = ["changes the AST", "rejects a file nu rejects"];

export const readComparison = (path: string) => JSON.parse(readFileSync(path, "utf8"));

const pairOf = (value: (formatter: keyof Pair) => number) => ({
  nuten: value("nuten"),
  nufmt: value("nufmt"),
});

export const directoryNamed = (comparison: Comparison, name: string) => {
  const directory = comparison.directories.find((entry) => entry.name === name);

  if (directory === undefined) {
    throw new Error(`the comparison times no directory named ${name}`);
  }

  return directory;
};

export const astChanges = (comparison: Comparison) => {
  const unchecked = comparison.excluded
    .filter((entry) => entry.formatter === "nuten" && !CHECKED_REASONS.includes(entry.reason))
    .map((entry) => entry.reason);

  if (unchecked.length > 0) {
    throw new Error(`nuten ${unchecked.join(", ")} on some files, so not every AST is checked`);
  }

  return pairOf((formatter) => {
    return comparison.excluded
      .filter((entry) => entry.formatter === formatter && entry.reason === "changes the AST")
      .reduce((total, entry) => total + entry.files.length, 0);
  });
};

const sorted = (values: number[]) => values.toSorted((a, b) => a - b);

export const median = (values: number[]) => {
  const ordered = sorted(values);
  const lower = ordered[Math.ceil(ordered.length / 2) - 1];
  const upper = ordered[Math.floor(ordered.length / 2)];

  if (lower === undefined || upper === undefined) {
    throw new Error("no values to take the median of");
  }

  return (lower + upper) / 2;
};

export const percentile = (values: number[], percent: number) => {
  const ordered = sorted(values);
  const rank = Math.max(Math.ceil((ordered.length * percent) / 100), 1);
  const value = ordered[rank - 1];

  if (value === undefined) {
    throw new Error("no values to take a percentile of");
  }

  return value;
};

export const instructionRatios = (comparison: Comparison) => {
  return comparison.timing.files.map((file) => file.instructions.nufmt / file.instructions.nuten);
};

export const medianInstructions = (comparison: Comparison) => {
  return pairOf((formatter) => {
    return median(comparison.timing.files.map((file) => file.instructions[formatter]));
  });
};

export const peakMemory = (comparison: Comparison) => {
  const perFile = pairOf((formatter) => {
    return median(comparison.timing.files.map((file) => file.memory_peak_resident[formatter]));
  });

  const allFiles = directoryNamed(comparison, "all").all_cpus.memory_peak_resident;

  return { perFile, allFiles };
};
