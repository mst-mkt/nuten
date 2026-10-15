import {
  type Commas,
  type IndentStyle,
  type LineEnding,
  format as formatInWasm,
} from "./gen/nuten.js";

export type RequiredOptions = {
  lineWidth: number;
  indentStyle: IndentStyle;
  indentWidth: number;
  lineEnding: LineEnding;
  commas: Commas;
};

export type Options = { [Key in keyof RequiredOptions]?: RequiredOptions[Key] | undefined };

export type ParseError = {
  line: number;
  column: number;
  message: string;
};

export class FormatError extends Error {
  override readonly name = "FormatError";
  readonly errors: readonly ParseError[];

  constructor(errors: readonly ParseError[]) {
    super(errors.map(({ line, column, message }) => `${line}:${column}: ${message}`).join("\n"));
    this.errors = errors;
  }
}

const optionKeys = {
  lineWidth: true,
  indentStyle: true,
  indentWidth: true,
  lineEnding: true,
  commas: true,
} as const satisfies Record<keyof Options, true>;

export const format = (source: string, options: Options = {}): string => {
  if (typeof source !== "string") {
    throw new TypeError("`source` takes a string");
  }
  if (!source.isWellFormed()) {
    throw new TypeError("`source` has a lone surrogate");
  }
  if (typeof options !== "object" || options === null || Array.isArray(options)) {
    throw new TypeError("`options` takes an object");
  }

  const unknownKey = Object.keys(options).find((key) => !Object.hasOwn(optionKeys, key));
  if (unknownKey !== undefined) {
    throw new TypeError(`unknown option \`${unknownKey}\``);
  }

  try {
    return formatInWasm(
      source,
      options.lineWidth,
      options.indentStyle,
      options.indentWidth,
      options.lineEnding,
      options.commas,
    );
  } catch (error) {
    if (typeof error === "string") {
      throw new TypeError(error);
    }
    if (Array.isArray(error)) {
      throw new FormatError(error.map(({ line, column, message }) => ({ line, column, message })));
    }

    throw error;
  }
};
