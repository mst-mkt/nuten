import { digits } from "./format";

export type Locale = "en" | "ja";

export const localeOf = (filePath: string) => {
  return /[\\/]content[\\/]ja[\\/]/.test(filePath) ? "ja" : "en";
};

const en = {
  speed: "Speed",
  speedDetail: (files: string) => `time to format ${files} files`,
  memory: "Memory",
  memoryDetail: (files: string) => `peak memory while formatting ${files} files`,
  size: "Binary size",
  sizeDetail: "size of the executable",
  correctness: "Correctness",
  correctnessDetail: (files: string) => `files out of ${files} whose AST changed`,

  faster: "faster",
  less: "less",
  smaller: "smaller",
  fewer: "fewer",
  unchangedAst: "no AST changes",
  fileCount: (count: number) => `${digits(count)} ${count === 1 ? "file" : "files"}`,

  allCpus: "All CPUs",
  oneCpu: "One CPU",
  perFile: "Single file, median",
  allFiles: "Overall, all CPUs",
  executable: "Executable",

  formatter: "Formatter",
  reason: "Reason",
  files: "Files",
  item: "Item",
  value: "Value",

  measured: "Measured",
  machine: "Machine",
  runs: "Runs",
  runsValue: (runs: number) => `${runs}, after one warmup run`,
  tools: "Tools",
  corpus: "Corpus",
  threads: (cpus: number) => `${cpus} threads`,

  instructions: "Instructions to format a single file, median",
  ratio: "nufmt / nuten",
  lowest: "Min",
  median: "Median",

  reasons: new Map([
    ["changes the AST", "Changes the AST"],
    ["rejects a file nu rejects", "Skips files with syntax errors"],
    ["fails", "Fails"],
    ["times out", "Times out"],
  ]),
};

type Labels = typeof en;

const ja: Labels = {
  speed: "速度",
  speedDetail: (files) => `${files} ファイルを整形する時間`,
  memory: "メモリ",
  memoryDetail: (files) => `${files} ファイルを整形するときの最大使用量`,
  size: "バイナリサイズ",
  sizeDetail: "実行ファイルの大きさ",
  correctness: "正しさ",
  correctnessDetail: (files) => `${files} ファイルのうち AST が変わったファイル`,

  faster: "高速",
  less: "省メモリ",
  smaller: "小型",
  fewer: "軽量",
  unchangedAst: "AST を変えない",
  fileCount: (count) => `${digits(count)} ファイル`,

  allCpus: "全 CPU",
  oneCpu: "1 CPU",
  perFile: "単一ファイル (中央値)",
  allFiles: "全体 (全 CPU)",
  executable: "実行ファイル",

  formatter: "フォーマッタ",
  reason: "理由",
  files: "ファイル数",
  item: "項目",
  value: "値",

  measured: "計測日",
  machine: "マシン",
  runs: "計測回数",
  runsValue: (runs) => `warmup 1 回の後に ${runs} 回`,
  tools: "計測ツール",
  corpus: "corpus",
  threads: (cpus) => `${cpus} スレッド`,

  instructions: "単一ファイルを整形する命令数 (中央値)",
  ratio: "nufmt / nuten",
  lowest: "最小",
  median: "中央値",

  reasons: new Map([
    ["changes the AST", "AST を変える"],
    ["rejects a file nu rejects", "構文エラーのため整形しない"],
    ["fails", "失敗する"],
    ["times out", "タイムアウトする"],
  ]),
};

export const LABELS = { en, ja } satisfies Record<Locale, Labels>;
