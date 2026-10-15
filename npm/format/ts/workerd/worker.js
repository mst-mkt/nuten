import { defaultOptions, format } from "./workerd.js";

export default {
  test: () => {
    const formatted = format("ls|sort");
    if (formatted !== "ls | sort\n") {
      throw new Error(`formatted to ${JSON.stringify(formatted)}`);
    }
    if (defaultOptions.lineWidth !== 100) {
      throw new Error(`default options are ${JSON.stringify(defaultOptions)}`);
    }
  },
};
