import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import path from "path";

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  base: "/admin/",
  server: {
    port: 5173,
    hmr: {
      port: 5173,
      protocol: "ws",
    },
    headers: {
      "Access-Control-Allow-Origin": "*",
    },
  },
  build: {
    outDir: "./dist",
    emptyOutDir: true,
  },
});
