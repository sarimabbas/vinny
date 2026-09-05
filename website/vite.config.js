import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";

const entry = (path) => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  build: {
    rollupOptions: {
      input: {
        main: entry("./index.html"),
        connection: entry("./guides/tigervnc-ssh/index.html"),
        display: entry("./guides/share-selected-display/index.html"),
        comparison: entry("./guides/vinny-vs-screen-sharing/index.html"),
      },
    },
  },
});
