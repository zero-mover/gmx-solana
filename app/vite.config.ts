import { defineConfig, loadEnv } from 'vite'
import react from '@vitejs/plugin-react'
import path from "path";
import { loadGMSOLDeployment } from "./utils/load-deployment";

export default defineConfig(async ({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  return {
    plugins: [react({
      babel: {
        plugins: ['macros'],
        compact: mode == "development" ? false : undefined,
      }
    })],
    resolve: {
      alias: {
        "styles": path.resolve(__dirname, "./src/styles"),
        "img": path.resolve(__dirname, "./src/img"),
        "components": path.resolve(__dirname, "./src/components"),
        "fonts": path.resolve(__dirname, "./src/fonts"),
        "config": path.resolve(__dirname, "./src/config"),
        "contexts": path.resolve(__dirname, "./src/contexts"),
        "states": path.resolve(__dirname, "./src/states"),
      }
    },
    define: {
      __GMSOL_DEPLOYMENT__: await loadGMSOLDeployment(env.GMSOL_DEPLOYMENT ? path.resolve(__dirname, env.GMSOL_DEPLOYMENT) : undefined),
    }
  }
})
