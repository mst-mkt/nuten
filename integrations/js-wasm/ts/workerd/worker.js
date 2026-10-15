import { defaultOptions, format, highlight } from "./workerd.js";

export default {
  test: () => {
    const formatted = format("ls|sort");
    if (formatted !== "ls | sort\n") {
      throw new Error(`formatted to ${JSON.stringify(formatted)}`);
    }
    if (defaultOptions.lineWidth !== 100) {
      throw new Error(`default options are ${JSON.stringify(defaultOptions)}`);
    }
    const highlights = JSON.stringify(highlight("ls"));
    if (highlights !== '[{"start":0,"end":2,"type":"function"}]') {
      throw new Error(`highlighted to ${highlights}`);
    }
  },
};
