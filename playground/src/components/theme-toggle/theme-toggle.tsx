import { Toggle } from "@base-ui/react/toggle";
import { Moon, Sun } from "lucide-react";

import { setTheme, useTheme } from "../../lib/theme";
import { IconButton } from "../icon-button/icon-button";

export const ThemeToggle = () => {
  const dark = useTheme() === "dark";

  return (
    <IconButton
      label="Dark theme"
      swapped={dark}
      icon={<Sun size={16} />}
      swappedIcon={<Moon size={16} />}
      render={
        <Toggle
          pressed={dark}
          onPressedChange={(pressed) => setTheme(pressed ? "dark" : "light")}
        />
      }
    />
  );
};
