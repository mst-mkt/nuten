import { Check, Copy } from "lucide-react";
import type { FC } from "react";

import { useCopy } from "../../lib/copy";
import { IconButton } from "../icon-button/icon-button";

type CopyButtonProps = {
  text: string;
};

export const CopyButton: FC<CopyButtonProps> = ({ text }) => {
  const { copied, copy } = useCopy("Output copied to clipboard");

  return (
    <IconButton
      label={copied ? "Copied" : "Copy"}
      swapped={copied}
      icon={<Copy size={16} />}
      swappedIcon={<Check size={16} />}
      onClick={() => void copy(text)}
    />
  );
};
