import { defineConfig } from "vite";

// Vite 构建配置, 由 Tauri 的 beforeDevCommand / beforeBuildCommand 调用
export default defineConfig({
  clearScreen: false,
  server: {
    // 固定开发服务器端口, 与 src-tauri/tauri.conf.json 中的 devUrl 保持一致
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2021",
    minify: false,
    sourcemap: false,
  },
});
