export const checkSource = (source: unknown): void => {
  if (typeof source !== "string") {
    throw new TypeError("`source` takes a string");
  }
  if (!source.isWellFormed()) {
    throw new TypeError("`source` has a lone surrogate");
  }
};
