import { Check, Copy } from "lucide-react";
import { type FC, useRef, useState } from "react";

import { IconButton } from "../icon-button/icon-button";

type CopyButtonProps = {
  text: string;
};

const copiedFor = 1500;

export const CopyButton: FC<CopyButtonProps> = ({ text }) => {
  const [copied, setCopied] = useState(false);
  const resetTimer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const copy = async () => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    clearTimeout(resetTimer.current);
    resetTimer.current = setTimeout(() => setCopied(false), copiedFor);
  };

  return (
    <IconButton
      label={copied ? "Copied" : "Copy"}
      swapped={copied}
      icon={<Copy size={16} />}
      swappedIcon={<Check size={16} />}
      onClick={() => void copy()}
    />
  );
};
