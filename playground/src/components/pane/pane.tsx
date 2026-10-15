import type { FC, ReactNode } from "react";

import styles from "./pane.module.css";

type PaneProps = {
  id?: string;
  titleId: string;
  title: string;
  grow: number;
  actions?: ReactNode;
  children: ReactNode;
};

export const Pane: FC<PaneProps> = ({ id, titleId, title, grow, actions, children }) => (
  <section id={id} className={styles.pane} style={{ flexGrow: grow }} aria-labelledby={titleId}>
    <header className={styles.header}>
      <h2 id={titleId} className={styles.title}>
        {title}
      </h2>
      {actions}
    </header>
    {children}
  </section>
);
