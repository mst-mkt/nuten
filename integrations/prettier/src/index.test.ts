import { execFile } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { type Options, format, getFileInfo } from "prettier";
import { describe, expect, onTestFinished, test } from "vite-plus/test";

import plugin from "./index";

const run = promisify(execFile);
const root = fileURLToPath(new URL("..", import.meta.url));

const block = "def f [] { [1 2 3] }";
const invalid = "ls\nfoo && bar || baz\n";

const formatNu = (source: string, options: Options = {}) =>
  format(source, { filepath: "main.nu", plugins: [plugin], ...options });

const listOfWidth = (width: number) => `[${"x".repeat(width - 5)}, x]`;

const writeTemporary = async (name: string, content: string) => {
  const directory = await mkdtemp(join(tmpdir(), "nuten-"));
  onTestFinished(() => rm(directory, { recursive: true }));

  const path = join(directory, name);
  await writeFile(path, content);

  return path;
};

describe("languages", () => {
  test.each(["nu", "nuon"])("%s_file_takes_nuten_parser", async (extension) => {
    const info = await getFileInfo(`main.${extension}`, { plugins: [plugin] });

    expect(info.inferredParser).toBe("nuten");
  });

  test("file_with_shebang_of_nu_takes_nuten_parser", async () => {
    const script = await writeTemporary("script", "#!/usr/bin/env nu\nls\n");

    const info = await getFileInfo(script, { plugins: [plugin] });

    expect(info.inferredParser).toBe("nuten");
  });

  test.each(["nu", "nushell"])("code_block_of_%s_in_markdown_is_formatted", async (language) => {
    const markdown = `\`\`\`${language}\nls|sort\n\`\`\`\n`;

    const formatted = await format(markdown, { parser: "markdown", plugins: [plugin] });

    expect(formatted).toBe(`\`\`\`${language}\nls | sort\n\`\`\`\n`);
  });
});

describe("package", () => {
  test("plugin_loads_by_package_name", async () => {
    const options = { filepath: "main.nu", plugins: ["@nuten/prettier-plugin"] };

    const formatted = await format("ls|sort", options);

    expect(formatted).toBe("ls | sort\n");
  });

  test("debug_check_passes_on_file_that_changes", async () => {
    const prettier = fileURLToPath(import.meta.resolve("prettier/bin/prettier.cjs"));
    const file = await writeTemporary("main.nu", "ls|sort\n");
    const args = [prettier, "--plugin", "@nuten/prettier-plugin", "--debug-check", file];

    const checking = run(process.execPath, args, { cwd: root });

    await expect(checking).resolves.toMatchObject({ stderr: "" });
  });
});

describe("options", () => {
  test("print_width_left_out_keeps_line_of_100_columns", async () => {
    const list = listOfWidth(100);

    const formatted = await formatNu(list);

    expect(formatted).toBe(`${list}\n`);
  });

  test("print_width_left_out_breaks_line_of_101_columns", async () => {
    const list = listOfWidth(101);

    const formatted = await formatNu(list);

    expect(formatted).toBe(`[\n    ${"x".repeat(96)}\n    x\n]\n`);
  });

  test("tab_width_left_out_takes_4", async () => {
    const formatted = await formatNu(block);

    expect(formatted).toBe("def f [] {\n    [1, 2, 3]\n}\n");
  });

  test("defaults_of_nuten_leave_other_languages_alone", async () => {
    const source = `${listOfWidth(81)};\n`;

    const formatted = await format(source, { parser: "babel", plugins: [plugin] });

    expect(formatted).toBe(`[\n  ${"x".repeat(76)},\n  x,\n];\n`);
  });

  test("print_width_sets_where_lines_break", async () => {
    const formatted = await formatNu(block, { printWidth: 10 });

    expect(formatted).toBe("def f [] {\n    [\n        1\n        2\n        3\n    ]\n}\n");
  });

  test("print_width_of_infinity_breaks_no_line", async () => {
    const list = listOfWidth(200);

    const formatted = await formatNu(list, { printWidth: Infinity });

    expect(formatted).toBe(`${list}\n`);
  });

  test("use_tabs_indents_with_tabs", async () => {
    const formatted = await formatNu(block, { useTabs: true });

    expect(formatted).toBe("def f [] {\n\t[1, 2, 3]\n}\n");
  });

  test("tab_width_sets_columns_per_indent", async () => {
    const formatted = await formatNu(block, { tabWidth: 2 });

    expect(formatted).toBe("def f [] {\n  [1, 2, 3]\n}\n");
  });

  test("end_of_line_sets_what_ends_each_line", async () => {
    const formatted = await formatNu(block, { endOfLine: "crlf" });

    expect(formatted).toBe("def f [] {\r\n    [1, 2, 3]\r\n}\r\n");
  });

  test("end_of_line_auto_follows_first_line_ending", async () => {
    const formatted = await formatNu("ls|sort\r\nls|sort\n", { endOfLine: "auto" });

    expect(formatted).toBe("ls | sort\r\nls | sort\r\n");
  });

  test("end_of_line_cr_is_rejected", async () => {
    const formatting = formatNu(block, { endOfLine: "cr" });

    await expect(formatting).rejects.toThrow(
      new TypeError("`endOfLine` takes one of `lf`, `crlf`, `auto`"),
    );
  });

  test("end_of_line_auto_following_cr_is_rejected", async () => {
    const source = 'let x = "a\rb"\nls\n';

    const formatting = formatNu(source, { endOfLine: "auto" });

    await expect(formatting).rejects.toThrow(
      new TypeError("`endOfLine` takes one of `lf`, `crlf`, `auto`"),
    );
  });

  test("nuten_commas_sets_where_commas_go", async () => {
    const formatted = await formatNu(block, { nutenCommas: "none" });

    expect(formatted).toBe("def f [] {\n    [1 2 3]\n}\n");
  });

  test.each([
    ["printWidth", "`printWidth` takes an integer from 1 to 4294967295"],
    ["tabWidth", "`tabWidth` takes an integer from 1 to 255"],
  ])("%s_out_of_range_is_rejected_by_name_in_prettier", async (option, message) => {
    const formatting = formatNu(block, { [option]: 0 });

    await expect(formatting).rejects.toThrow(new TypeError(message));
  });
});

describe("pragmas", () => {
  test("require_pragma_leaves_file_without_pragma", async () => {
    const formatted = await formatNu("ls|sort\n", { requirePragma: true });

    expect(formatted).toBe("ls|sort\n");
  });

  test("require_pragma_leaves_file_with_pragma_after_code", async () => {
    const source = "ls|sort\n# @format\n";

    const formatted = await formatNu(source, { requirePragma: true });

    expect(formatted).toBe(source);
  });

  test.each(["@format", "@prettier"])("require_pragma_formats_file_with_%s", async (pragma) => {
    const source = `#!/usr/bin/env nu\n# ${pragma}\nls|sort\n`;

    const formatted = await formatNu(source, { requirePragma: true });

    expect(formatted).toBe(`#!/usr/bin/env nu\n# ${pragma}\nls | sort\n`);
  });

  test.each(["@noformat", "@noprettier"])(
    "check_ignore_pragma_leaves_file_with_%s",
    async (pragma) => {
      const source = `# ${pragma}\nls|sort\n`;

      const formatted = await formatNu(source, { checkIgnorePragma: true });

      expect(formatted).toBe(source);
    },
  );

  test("insert_pragma_puts_pragma_at_top", async () => {
    const formatted = await formatNu("ls|sort\n", { insertPragma: true });

    expect(formatted).toBe("# @format\n\nls | sort\n");
  });

  test("insert_pragma_puts_pragma_after_shebang", async () => {
    const formatted = await formatNu("#!/usr/bin/env nu\nls|sort\n", { insertPragma: true });

    expect(formatted).toBe("#!/usr/bin/env nu\n# @format\n\nls | sort\n");
  });

  test("insert_pragma_leaves_pragma_already_there", async () => {
    const source = "# @format\n\nls | sort\n";

    const formatted = await formatNu(source, { insertPragma: true });

    expect(formatted).toBe(source);
  });
});

describe("syntax errors", () => {
  test("syntax_error_points_at_first_one", async () => {
    const formatting = formatNu(invalid);

    await expect(formatting).rejects.toMatchObject({
      name: "SyntaxError",
      message: expect.stringContaining(
        "`&&` is not an operator; use `and` (`;` for the shell meaning) (2:5)\n",
      ),
      loc: { start: { line: 2, column: 5 } },
    });
  });

  test("syntax_error_keeps_every_one_in_cause", async () => {
    const formatting = formatNu(invalid);

    await expect(formatting).rejects.toMatchObject({
      cause: {
        errors: [
          { line: 2, column: 5 },
          { line: 2, column: 12 },
        ],
      },
    });
  });

  test("syntax_error_column_counts_utf16_code_units", async () => {
    const formatting = formatNu("'🧶' && x");

    await expect(formatting).rejects.toMatchObject({
      message: expect.stringContaining("(1:6)"),
      loc: { start: { line: 1, column: 6 } },
    });
  });

  test("syntax_error_column_counts_code_units_of_its_own_line", async () => {
    const formatting = formatNu("'🧶'\nls && x");

    await expect(formatting).rejects.toMatchObject({ loc: { start: { line: 2, column: 4 } } });
  });
});
