import type { FC, ReactNode } from "react";

import styles from "./pane.module.css";

type PaneProps = {
  titleId: string;
  title: string;
  actions?: ReactNode;
  children: ReactNode;
};

export const Pane: FC<PaneProps> = ({ titleId, title, actions, children }) => (
  <section className={styles.pane} aria-labelledby={titleId}>
    <header className={styles.header}>
      <h2 id={titleId} className={styles.title}>
        {title}
      </h2>
      {actions}
    </header>
    {children}
  </section>
);
