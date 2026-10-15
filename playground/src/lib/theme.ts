import { useSyncExternalStore } from "react";

export type Theme = "light" | "dark";

const storageKey = "theme";
const darkScheme = "(prefers-color-scheme: dark)";

const themeOf = (): Theme => {
  const chosen = document.documentElement.dataset.theme;
  if (chosen === "light" || chosen === "dark") return chosen;

  return matchMedia(darkScheme).matches ? "dark" : "light";
};

const subscribe = (onChange: () => void) => {
  const scheme = matchMedia(darkScheme);
  const observer = new MutationObserver(onChange);

  scheme.addEventListener("change", onChange);
  observer.observe(document.documentElement, { attributeFilter: ["data-theme"] });

  return () => {
    scheme.removeEventListener("change", onChange);
    observer.disconnect();
  };
};

export const useTheme = () => {
  return useSyncExternalStore(subscribe, themeOf);
};

const applyTheme = (theme: Theme) => {
  document.documentElement.dataset.theme = theme;

  try {
    localStorage.setItem(storageKey, theme);
  } catch {}
};

export const setTheme = (theme: Theme) => {
  const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (reducedMotion || document.startViewTransition === undefined) {
    applyTheme(theme);
    return;
  }

  document.startViewTransition(() => applyTheme(theme));
};
