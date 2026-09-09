import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and fails fast rather than silently moving.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    // The shell loads this build output; see crates/epoch-tauri/tauri.conf.json.
    outDir: "dist",
    emptyOutDir: true,
  },
});
