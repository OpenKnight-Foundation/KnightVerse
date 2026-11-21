import * as React from "react";
import "../app/globals.css";

type StoryComponent = React.ComponentType;
type PreviewContext = { globals: { theme?: string } };

const preview = {
  parameters: {
    layout: "centered",
    backgrounds: {
      default: "knightverse-dark",
      values: [
        { name: "knightverse-dark", value: "#030712" },
        { name: "knightverse-light", value: "#f8fafc" },
      ],
    },
  },
  globalTypes: {
    theme: {
      description: "KnightVerse color theme",
      defaultValue: "dark",
      toolbar: {
        icon: "circlehollow",
        items: ["light", "dark"],
        dynamicTitle: true,
      },
    },
  },
  decorators: [
    (Story: StoryComponent, context: PreviewContext) => {
      const theme = context.globals.theme === "light" ? "light" : "dark";
      if (typeof document !== "undefined") {
        document.documentElement.classList.toggle("dark", theme === "dark");
        document.documentElement.style.colorScheme = theme;
      }
      return React.createElement(Story);
    },
  ],
};

export default preview;
