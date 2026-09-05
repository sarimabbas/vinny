import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";

const entry = (path) => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  build: {
    rollupOptions: {
      input: {
        main: entry("./index.html"),
        notFound: entry("./404.html"),
        connection: entry("./guides/connection/index.html"),
        legacyConnection: entry("./guides/tigervnc-ssh/index.html"),
        settings: entry("./guides/settings/index.html"),
        legacyDisplay: entry("./guides/share-selected-display/index.html"),
        tailscale: entry("./guides/tailscale/index.html"),
        comparison: entry("./guides/vinny-vs-screen-sharing/index.html"),
      },
    },
  },
});
