export const digits = (value: number, fraction = 0) => {
  const [whole = "", decimals] = value.toFixed(fraction).split(".");
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ",");

  return decimals === undefined ? grouped : `${grouped}.${decimals}`;
};

export const milliseconds = (nanoseconds: number) => {
  const ms = nanoseconds / 1_000_000;
  const fraction = Number.parseFloat(ms.toFixed(1)) < 10 ? 1 : 0;

  return `${digits(ms, fraction)} ms`;
};

export const mebibytes = (bytes: number) => `${digits(bytes / 1_048_576, 1)} MiB`;

export const times = (ratio: number) => `${digits(ratio, 1)}x`;

export const utcDate = (date: string) => new Date(date).toISOString().slice(0, 10);
