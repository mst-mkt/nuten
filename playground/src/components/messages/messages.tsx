import type { FC } from "react";

import type { Message } from "../../lib/format";
import styles from "./messages.module.css";

type MessagesProps = {
  messages: Message[];
};

export const Messages: FC<MessagesProps> = ({ messages }) => (
  <ul className={styles.messages}>
    {messages.map(({ position, text }) => (
      <li key={`${position?.line}:${position?.column}:${text}`} className={styles.message}>
        {position !== undefined && (
          <span className={styles.place}>
            {position.line}:{position.column}
          </span>
        )}
        <span className={styles.text}>{text}</span>
      </li>
    ))}
  </ul>
);
