import { defineConfig } from "@rsbuild/core";
import { pluginVue } from "@rsbuild/plugin-vue";

export default defineConfig({
  plugins: [pluginVue()],
  source: {
    entry: {
      index: "./src/main.js",
    },
    alias: {
      "@": "./src",
    },
  },
  server: {
    port: 5173,
    headers: {
      "Access-Control-Allow-Origin": "*",
    },
  },
  dev: {
    hmr: {
      port: 5173,
      protocol: "ws",
    },
  },
  html: {
    publicPath: "/admin/",
  },
  output: {
    distPath: {
      root: "./dist",
    },
    clean: true,
  },
});
