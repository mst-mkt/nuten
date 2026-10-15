import { Collapsible } from "@base-ui/react/collapsible";
import { Fieldset } from "@base-ui/react/fieldset";
import type { RequiredOptions } from "@nuten/format";
import { ChevronDown } from "lucide-react";
import type { FC } from "react";

import { NumberInput } from "../number-input/number-input";
import { Select } from "../select/select";
import { commasLabels, indentStyleLabels, lineEndingLabels } from "./labels";
import styles from "./settings.module.css";
import { summaryOf } from "./summary";

type SettingsProps = {
  options: RequiredOptions;
  onChange: (options: RequiredOptions) => void;
};

export const Settings: FC<SettingsProps> = ({ options, onChange }) => (
  <Collapsible.Root className={styles.settings}>
    <Collapsible.Trigger className={styles.trigger}>
      <span className={styles.heading}>Settings</span>
      <span className={styles.summary}>{summaryOf(options)}</span>
      <ChevronDown className={styles.chevron} size={16} />
    </Collapsible.Trigger>
    <Collapsible.Panel className={styles.panel} keepMounted>
      <Fieldset.Root className={styles.fields}>
        <Fieldset.Legend className={styles.legend}>Settings</Fieldset.Legend>
        <NumberInput
          name="lineWidth"
          label="Line width"
          value={options.lineWidth}
          min={1}
          onChange={(lineWidth) => onChange({ ...options, lineWidth })}
        />
        <Select
          name="indentStyle"
          label="Indent style"
          value={options.indentStyle}
          options={indentStyleLabels}
          onChange={(indentStyle) => onChange({ ...options, indentStyle })}
        />
        <NumberInput
          name="indentWidth"
          label="Indent width"
          value={options.indentWidth}
          min={1}
          onChange={(indentWidth) => onChange({ ...options, indentWidth })}
        />
        <Select
          name="lineEnding"
          label="Line ending"
          value={options.lineEnding}
          options={lineEndingLabels}
          onChange={(lineEnding) => onChange({ ...options, lineEnding })}
        />
        <Select
          name="commas"
          label="Commas"
          value={options.commas}
          options={commasLabels}
          onChange={(commas) => onChange({ ...options, commas })}
        />
      </Fieldset.Root>
    </Collapsible.Panel>
  </Collapsible.Root>
);
