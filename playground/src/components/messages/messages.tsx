import type { FC } from "react";

import type { Message, Position } from "../../lib/format";
import styles from "./messages.module.css";

type MessagesProps = {
  messages: Message[];
  onSelect: (position: Position) => void;
};

export const Messages: FC<MessagesProps> = ({ messages, onSelect }) => (
  <ul className={styles.messages}>
    {messages.map(({ position, text }) => (
      <li key={`${position?.line}:${position?.column}:${text}`}>
        {position === undefined ? (
          <div className={styles.message}>
            <span className={styles.text}>{text}</span>
          </div>
        ) : (
          <button type="button" className={styles.message} onClick={() => onSelect(position)}>
            <span className={styles.place}>
              {position.line}:{position.column}
            </span>
            <span className={styles.text}>{text}</span>
          </button>
        )}
      </li>
    ))}
  </ul>
);
