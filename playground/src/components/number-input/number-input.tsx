import { Field } from "@base-ui/react/field";
import { NumberField } from "@base-ui/react/number-field";
import { Minus, Plus } from "lucide-react";
import type { FC } from "react";

import styles from "./number-input.module.css";

type NumberInputProps = {
  name: string;
  label: string;
  value: number;
  min: number;
  onChange: (value: number) => void;
};

export const NumberInput: FC<NumberInputProps> = ({ name, label, value, min, onChange }) => (
  <Field.Root className={styles.root} name={name}>
    <NumberField.Root
      className={styles.field}
      value={value}
      min={min}
      onValueChange={(next) => {
        if (next !== null) onChange(next);
      }}
    >
      <Field.Label className={styles.label}>{label}</Field.Label>
      <NumberField.Group className={styles.group}>
        <NumberField.Decrement className={styles.stepper}>
          <Minus size={16} />
        </NumberField.Decrement>
        <NumberField.Input className={styles.input} />
        <NumberField.Increment className={styles.stepper}>
          <Plus size={16} />
        </NumberField.Increment>
      </NumberField.Group>
    </NumberField.Root>
  </Field.Root>
);
