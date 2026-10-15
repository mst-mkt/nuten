import { Check, Share } from "lucide-react";
import type { FC } from "react";

import { useCopy } from "../../lib/copy";
import { IconButton } from "../icon-button/icon-button";

type ShareButtonProps = {
  urlOf: () => string;
};

export const ShareButton: FC<ShareButtonProps> = ({ urlOf }) => {
  const { copied, copy } = useCopy("Link copied to clipboard");

  return (
    <IconButton
      label={copied ? "Copied" : "Share"}
      swapped={copied}
      icon={<Share size={16} />}
      swappedIcon={<Check size={16} />}
      onClick={() => void copy(urlOf())}
    />
  );
};
