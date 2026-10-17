import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  base: "./",
  plugins: [react(), tailwindcss()],
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes("node_modules/katex")) return "katex";
          if (id.includes("node_modules/recharts")) return "recharts";
          if (id.includes("node_modules/docx")) return "docx";
          if (id.includes("node_modules/pptxgenjs")) return "pptx";
        },
      },
    },
  },
  server: {
    // Keep Suna separate from the legacy web app that may still occupy 5173.
    port: 1420,
    host: "127.0.0.1",
    proxy: {
      "/api": {
        target: "http://localhost:8000",
        changeOrigin: true,
      },
    },
  },
});
