const config = {
  stories: ["../components/**/*.stories.@(ts|tsx)"],
  addons: [],
  framework: { name: "@storybook/nextjs", options: {} },
  staticDirs: ["../public"],
};

export default config;
