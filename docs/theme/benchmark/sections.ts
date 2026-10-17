import { barGroups, type BarGroup } from "./bars";
import {
  astChanges,
  directoryNamed,
  instructionRatios,
  medianInstructions,
  peakMemory,
  percentile,
  type Comparison,
  type Locked,
  type Pair,
} from "./comparison";
import { digits, mebibytes, milliseconds, times, utcDate } from "./format";
import { element, table, type Child } from "./html";
import { LABELS, type Locale } from "./labels";

type Section = (comparison: Comparison, locale: Locale, argument?: string) => Child;

const lead = (values: Pair, comparative: string) => {
  const ratio = values.nufmt / values.nuten;

  return Number.isFinite(ratio) && ratio > 1 ? `${times(ratio)} ${comparative}` : undefined;
};

const pairTable = (groups: BarGroup[]) => {
  const head = ["", ...groups.map((group) => group.label)];
  const row = (formatter: keyof Pair) => [
    formatter,
    ...groups.map(({ values, format }) => format(values[formatter])),
  ];

  return table(head, [row("nuten"), row("nufmt")]);
};

const overview: Section = (comparison, locale, layout) => {
  const labels = LABELS[locale];
  const directory = directoryNamed(comparison, "all");
  const compared = digits(directory.files);

  const speed = directory.all_cpus.time_wall_clock;
  const memory = directory.all_cpus.memory_peak_resident;
  const changes = astChanges(comparison);

  return barGroups(
    [
      {
        name: labels.speed,
        label: labels.speedDetail(compared),
        values: speed,
        format: milliseconds,
        note: lead(speed, labels.faster),
      },
      {
        name: labels.memory,
        label: labels.memoryDetail(compared),
        values: memory,
        format: mebibytes,
        note: lead(memory, labels.less),
      },
      {
        name: labels.size,
        label: labels.sizeDetail,
        values: comparison.sizes,
        format: mebibytes,
        note: lead(comparison.sizes, labels.smaller),
      },
      {
        name: labels.correctness,
        label: labels.correctnessDetail(digits(comparison.files)),
        values: changes,
        format: labels.fileCount,
        note: changes.nuten === 0 ? labels.unchangedAst : undefined,
      },
    ],
    layout,
  );
};

const directory: Section = (comparison, locale, name = "all") => {
  const labels = LABELS[locale];
  const { all_cpus, one_cpu } = directoryNamed(comparison, name);
  const oneCpu = one_cpu?.time_wall_clock;
  const groups = [
    { label: labels.allCpus, values: all_cpus.time_wall_clock, format: milliseconds },
    ...(oneCpu === undefined
      ? []
      : [{ label: labels.oneCpu, values: oneCpu, format: milliseconds }]),
  ];

  return [barGroups(groups), pairTable(groups)];
};

const single: Section = (comparison, locale) => {
  const labels = LABELS[locale];
  const ratios = instructionRatios(comparison);

  const instructions = medianInstructions(comparison);
  const bars = barGroups([
    {
      label: labels.instructions,
      values: instructions,
      format: digits,
      note: lead(instructions, labels.fewer),
    },
  ]);

  const percentiles = [
    { name: labels.lowest, percent: 0 },
    { name: "p10", percent: 10 },
    { name: labels.median, percent: 50 },
    { name: "p90", percent: 90 },
  ];
  const percentileTable = table(
    ["", ...percentiles.map(({ name }) => name)],
    [[labels.ratio, ...percentiles.map(({ percent }) => times(percentile(ratios, percent)))]],
  );

  return [bars, percentileTable];
};

const memory: Section = (comparison, locale) => {
  const labels = LABELS[locale];
  const { perFile, allFiles } = peakMemory(comparison);
  const groups = [
    { label: labels.perFile, values: perFile, format: mebibytes },
    { label: labels.allFiles, values: allFiles, format: mebibytes },
  ];

  return [barGroups(groups), pairTable(groups)];
};

const size: Section = (comparison, locale) => {
  const groups = [
    { label: LABELS[locale].executable, values: comparison.sizes, format: mebibytes },
  ];

  return [barGroups(groups), pairTable(groups)];
};

const excluded: Section = (comparison, locale) => {
  if (comparison.excluded.length === 0) return undefined;

  const labels = LABELS[locale];
  const reason = (key: string) => labels.reasons.get(key) ?? key;

  const rows = comparison.excluded.map((entry) => [
    entry.formatter,
    reason(entry.reason),
    digits(entry.files.length),
  ]);
  const lists = comparison.excluded.map((entry) => {
    const summary = `${entry.formatter}: ${reason(entry.reason)} (${digits(entry.files.length)})`;
    const files = entry.files.map((file) => element("li", {}, element("code", {}, file)));

    return element("details", {}, element("summary", {}, summary), element("ul", {}, files));
  });

  return [table([labels.formatter, labels.reason, labels.files], rows), lists];
};

const conditions: Section = (comparison, locale) => {
  const labels = LABELS[locale];
  const { versions, environment } = comparison;

  const locked = (lock: Locked | null) => {
    return lock === null ? "" : `${lock.rev.slice(0, 7)} (${utcDate(lock.date)})`;
  };
  const cpu = environment.cpu
    .replace(/\((r|tm)\)|[®™]/gi, "")
    .replace(/\s+/g, " ")
    .trim();
  const corpus = Object.entries(versions.corpus).map(([name, lock]) => {
    return element("div", {}, `${name} ${locked(lock)}`);
  });

  const rows = [
    [labels.measured, `${utcDate(comparison.measured_at)} (UTC)`],
    ["nuten", versions.nuten ?? ""],
    ["nufmt", locked(versions.nufmt)],
    ["Nushell", locked(versions.nushell)],
    [labels.machine, `${cpu}, ${labels.threads(environment.cpus)}, ${environment.os}`],
    [labels.runs, labels.runsValue(comparison.runs)],
    [labels.tools, "hyperfine, valgrind"],
    [labels.corpus, corpus],
  ];

  return table([labels.item, labels.value], rows);
};

export const SECTIONS = new Map([
  ["overview", overview],
  ["directory", directory],
  ["single", single],
  ["memory", memory],
  ["size", size],
  ["excluded", excluded],
  ["conditions", conditions],
]);
