import { FormatError, type Options, type ParseError, format } from "@nuten/wasm";

export type Position = Pick<ParseError, "line" | "column">;

export type Message = {
  position?: Position;
  text: string;
};

export type FormatResult =
  | { kind: "output"; output: string }
  | { kind: "syntax-errors"; errors: readonly ParseError[] }
  | { kind: "invalid-input"; message: string }
  | { kind: "failure"; message: string };

export const formatResultOf = (source: string, options: Options): FormatResult => {
  try {
    return { kind: "output", output: format(source, options) };
  } catch (error) {
    if (error instanceof FormatError) {
      return { kind: "syntax-errors", errors: error.errors };
    }
    if (error instanceof TypeError) {
      return { kind: "invalid-input", message: error.message };
    }
    return { kind: "failure", message: String(error) };
  }
};

export const messagesOf = (result: FormatResult): Message[] => {
  if (result.kind === "output") return [];
  if (result.kind === "syntax-errors") {
    return result.errors.map(({ line, column, message }) => {
      return { position: { line, column }, text: message };
    });
  }

  return [{ text: result.message }];
};
