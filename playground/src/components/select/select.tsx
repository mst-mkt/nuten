import { Select as BaseSelect } from "@base-ui/react/select";
import { Check, ChevronDown } from "lucide-react";

import styles from "./select.module.css";

type SelectProps<Value extends string> = {
  name: string;
  label: string;
  value: Value;
  options: Record<Value, string>;
  onChange: (value: Value) => void;
};

export const Select = <Value extends string>({
  name,
  label,
  value,
  options,
  onChange,
}: SelectProps<Value>) => (
  <BaseSelect.Root
    name={name}
    items={options}
    value={value}
    onValueChange={(next) => {
      if (next !== null) onChange(next);
    }}
  >
    <div className={styles.root}>
      <BaseSelect.Label className={styles.label}>{label}</BaseSelect.Label>
      <BaseSelect.Trigger className={styles.trigger}>
        <BaseSelect.Value />
        <BaseSelect.Icon className={styles.icon}>
          <ChevronDown size={16} />
        </BaseSelect.Icon>
      </BaseSelect.Trigger>
    </div>
    <BaseSelect.Portal>
      <BaseSelect.Positioner
        className={styles.positioner}
        sideOffset={6}
        alignItemWithTrigger={false}
      >
        <BaseSelect.Popup className={styles.popup}>
          <BaseSelect.List>
            {Object.entries<string>(options).map(([option, optionLabel]) => (
              <BaseSelect.Item key={option} value={option} className={styles.item}>
                <BaseSelect.ItemText>{optionLabel}</BaseSelect.ItemText>
                <BaseSelect.ItemIndicator className={styles.indicator}>
                  <Check size={16} />
                </BaseSelect.ItemIndicator>
              </BaseSelect.Item>
            ))}
          </BaseSelect.List>
        </BaseSelect.Popup>
      </BaseSelect.Positioner>
    </BaseSelect.Portal>
  </BaseSelect.Root>
);
