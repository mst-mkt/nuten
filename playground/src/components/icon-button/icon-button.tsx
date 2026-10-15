import { mergeProps } from "@base-ui/react/merge-props";
import { Tooltip } from "@base-ui/react/tooltip";
import { useRender } from "@base-ui/react/use-render";
import type { FC, ReactNode } from "react";

import styles from "./icon-button.module.css";

type IconButtonProps = useRender.ComponentProps<"button"> & {
  label: string;
  swapped: boolean;
  icon: ReactNode;
  swappedIcon: ReactNode;
};

export const IconButton: FC<IconButtonProps> = ({
  label,
  swapped,
  icon,
  swappedIcon,
  render,
  ...props
}) => {
  const button = useRender({
    defaultTagName: "button",
    render,
    props: mergeProps<"button">(
      {
        type: "button",
        className: styles.button,
        "aria-label": label,
        children: (
          <span className={styles.icons}>
            <span className={styles.icon} data-visible={!swapped}>
              {icon}
            </span>
            <span className={styles.icon} data-visible={swapped}>
              {swappedIcon}
            </span>
          </span>
        ),
      },
      props,
    ),
  });

  return (
    <Tooltip.Root>
      <Tooltip.Trigger render={button} />
      <Tooltip.Portal>
        <Tooltip.Positioner sideOffset={6}>
          <Tooltip.Popup className={styles.tooltip}>{label}</Tooltip.Popup>
        </Tooltip.Positioner>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
};
