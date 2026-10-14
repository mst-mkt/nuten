import { format } from "./workerd.js";

export default {
  test: () => {
    const formatted = format("ls|sort");
    if (formatted !== "ls | sort\n") {
      throw new Error(`formatted to ${JSON.stringify(formatted)}`);
    }
  },
};
