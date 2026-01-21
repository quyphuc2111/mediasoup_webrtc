import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1421,
    strictPort: true,
  },
  build: {
    outDir: "dist-student",
    rollupOptions: {
      input: "./index-student.html",
      output: {
        entryFileNames: "assets/[name]-[hash].js",
      },
    },
  },
  publicDir: "public",
});
