import { type FC, type KeyboardEvent, type PointerEvent, useCallback, useState } from "react";

import styles from "./splitter.module.css";

const MIN_RATIO = 0.15;
const MAX_RATIO = 0.85;
const DEFAULT_RATIO = 0.5;
const KEY_STEP = 0.05;

const clamp = (ratio: number) => Math.min(MAX_RATIO, Math.max(MIN_RATIO, ratio));

type SplitterProps = {
  ratio: number;
  onChange: (ratio: number) => void;
  label: string;
  controls: string;
};

export const Splitter: FC<SplitterProps> = ({ ratio, onChange, label, controls }) => {
  const [stacked, setStacked] = useState(false);

  const observeShape = useCallback((handle: HTMLDivElement) => {
    const observer = new ResizeObserver(() => {
      setStacked(handle.offsetWidth > handle.offsetHeight);
    });
    observer.observe(handle);
    return () => observer.disconnect();
  }, []);

  const ratioAt = (handle: HTMLElement, event: PointerEvent) => {
    const container = handle.parentElement;
    if (container === null) return ratio;

    const rect = container.getBoundingClientRect();
    const [offset, size, gutter] = stacked
      ? [event.clientY - rect.top, rect.height, handle.offsetHeight]
      : [event.clientX - rect.left, rect.width, handle.offsetWidth];

    return clamp((offset - gutter / 2) / (size - gutter));
  };

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
    onChange(ratioAt(event.currentTarget, event));
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const [decrease, increase] = stacked ? ["ArrowUp", "ArrowDown"] : ["ArrowLeft", "ArrowRight"];
    const next = {
      [decrease]: ratio - KEY_STEP,
      [increase]: ratio + KEY_STEP,
      Home: MIN_RATIO,
      End: MAX_RATIO,
      Enter: DEFAULT_RATIO,
    }[event.key];

    if (next === undefined) return;

    event.preventDefault();
    onChange(clamp(next));
  };

  return (
    <div
      ref={observeShape}
      className={styles.splitter}
      role="separator"
      tabIndex={0}
      aria-label={label}
      aria-controls={controls}
      aria-orientation={stacked ? "horizontal" : "vertical"}
      aria-valuemin={Math.round(MIN_RATIO * 100)}
      aria-valuemax={Math.round(MAX_RATIO * 100)}
      aria-valuenow={Math.round(ratio * 100)}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onKeyDown={onKeyDown}
      onDoubleClick={() => onChange(DEFAULT_RATIO)}
    />
  );
};
