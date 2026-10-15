import { FormatError, type Options, format } from "@nuten/format";
import type { Plugin } from "prettier";

type Commas = Options["commas"];

declare module "prettier" {
  interface RequiredOptions {
    nutenCommas: Commas;
  }
}

const maxLineWidth = 2 ** 32 - 1;

const hasLeadingPragma = (text: string, pragmas: string[]) => {
  const leadingComments = /^(?:#.*\n?)*/.exec(text)?.at(0) ?? "";
  const comments = leadingComments.split("\n");

  return comments.some((comment) => pragmas.includes(comment.slice(1).trim()));
};

const insertPragma = (text: string) => {
  const shebangEnd = text.startsWith("#!") ? text.indexOf("\n") + 1 : 0;
  const shebang = text.slice(0, shebangEnd);
  const body = text.slice(shebangEnd);

  return `${shebang}# @format\n\n${body}`;
};

const syntaxErrorOf = (text: string, cause: FormatError) => {
  const parseError = cause.errors.at(0);
  if (parseError === undefined) {
    return cause;
  }

  const { line, column, message } = parseError;

  const lineText = text.split("\n").at(line - 1) ?? "";
  const codePoints = Array.from(lineText);
  const before = codePoints.slice(0, column - 1).join("");
  const utf16Column = before.length + 1;

  const error = new SyntaxError(`${message} (${line}:${utf16Column})`, { cause });
  const loc = { start: { line, column: utf16Column } };

  return Object.assign(error, { loc });
};

const optionErrorOf = (error: TypeError) => {
  const message = error.message
    .replace("`lineWidth`", "`printWidth`")
    .replace("`indentWidth`", "`tabWidth`");

  return new TypeError(message);
};

const plugin: Plugin<{ text: string; formatted: string }> = {
  languages: [
    {
      name: "Nushell",
      parsers: ["nuten"],
      extensions: [".nu"],
      interpreters: ["nu"],
      vscodeLanguageIds: ["nushell"],
    },
    {
      name: "NUON",
      parsers: ["nuten"],
      extensions: [".nuon"],
      vscodeLanguageIds: ["nuon"],
    },
  ],
  defaultOptions: {
    printWidth: 100,
    tabWidth: 4,
  },
  options: {
    nutenCommas: {
      category: "Nuten",
      type: "choice",
      default: "inline",
      description: "Where commas go between the items of lists, records, and the like.",
      choices: [
        { value: "none", description: "No commas." },
        { value: "inline", description: "Commas between items on one line." },
        { value: "always", description: "Commas between items, also across lines." },
        {
          value: "trailing",
          description: "Commas between items, and after the last one across lines.",
        },
      ] satisfies { value: Commas; description: string }[],
    },
  },
  parsers: {
    nuten: {
      parse: (text, options) => {
        if (options.endOfLine === "cr") {
          throw new TypeError("`endOfLine` takes one of `lf`, `crlf`, `auto`");
        }

        try {
          const formatted = format(text, {
            lineWidth: Math.min(options.printWidth, maxLineWidth),
            indentStyle: options.useTabs ? "tab" : "space",
            indentWidth: options.tabWidth,
            lineEnding: "lf",
            commas: options.nutenCommas,
          });

          return { text, formatted };
        } catch (error) {
          if (error instanceof TypeError) {
            throw optionErrorOf(error);
          }
          if (error instanceof FormatError) {
            throw syntaxErrorOf(text, error);
          }

          throw error;
        }
      },
      astFormat: "nuten",
      locStart: () => 0,
      locEnd: ({ text }) => text.length,
      hasPragma: (text) => hasLeadingPragma(text, ["@format", "@prettier"]),
      hasIgnorePragma: (text) => hasLeadingPragma(text, ["@noformat", "@noprettier"]),
    },
  },
  printers: {
    nuten: {
      print: ({ node }) => node.formatted,
      insertPragma,
      massageAstNode: ({ formatted }) => ({ formatted }),
    },
  },
};

export default plugin;
