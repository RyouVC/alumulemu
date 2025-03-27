import { defineConfig } from '@rsbuild/core';
import { pluginVue } from '@rsbuild/plugin-vue';

export default defineConfig({

  plugins: [pluginVue()],
  resolve: {
    extensions: ['.ts', '.tsx', '.mjs', '.js', '.jsx', '.json'],
    // extensionAlias: {
    //   '.js': ['.ts', '.js'],
    // },
  },
  source: {
    entry: {
      index: "./src/main.ts",
    },
    alias: {
      "@": "./src",
    },
  },
  server: {
    printUrls: true,
    port: 5173,
    headers: {
      "Access-Control-Allow-Origin": "*",
    },
    base: "/",
    proxy: {
      "/api": "http://localhost:3000",
      "/admin": "http://localhost:3000",
    },
  },
  dev: {
    liveReload: true,
    hmr: true
    // hmr: {
    //   port: 5173,
    //   protocol: "ws",
    // },
  },
  html: {
    template: "./index.html",

  },
  output: {
    distPath: {
      root: "./dist",
    },
    cleanDistPath: true,

  },
});
