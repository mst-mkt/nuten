import { useRef, useState } from "react";

import { notify } from "./toasts";

const copiedFor = 1500;

export const useCopy = (copiedMessage: string) => {
  const [copied, setCopied] = useState(false);
  const resetTimer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      notify("Could not copy to clipboard", "error");
      return;
    }

    notify(copiedMessage);
    setCopied(true);
    clearTimeout(resetTimer.current);
    resetTimer.current = setTimeout(() => setCopied(false), copiedFor);
  };

  return { copied, copy };
};
