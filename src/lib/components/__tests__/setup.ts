/// <reference types="bun-types" />
import { plugin } from "bun";
import { compile, compileModule } from "svelte/compiler";
import { readFileSync } from "fs";
import { JSDOM } from "jsdom";
import { join } from "path";

// Register Svelte plugin for Bun to load .svelte and .svelte.js/ts files
plugin({
  name: "svelte-loader",
  setup(build: any) {
    // Redirect svelte imports to browser/client versions
    build.onResolve({ filter: /^svelte(\/|$)/ }, (args: any) => {
      if (args.path === "svelte") {
        return {
          path: join(process.cwd(), "node_modules/svelte/src/index-client.js")
        };
      }
      if (args.path === "svelte/reactivity") {
        return {
          path: join(process.cwd(), "node_modules/svelte/src/reactivity/index-client.js")
        };
      }
      return null; // Let Bun handle other paths normally
    });

    // Process .svelte components
    build.onLoad({ filter: /\.svelte$/ }, ({ path }: { path: string }) => {
      const source = readFileSync(path, "utf8");
      const { js } = compile(source, {
        filename: path,
        generate: "client"
      });
      return {
        contents: js.code,
        loader: "js",
      };
    });

    // Process .svelte.js and .svelte.ts scripts (e.g. from node_modules/@testing-library/svelte)
    build.onLoad({ filter: /\.svelte\.(js|ts)$/ }, ({ path }: { path: string }) => {
      const source = readFileSync(path, "utf8");
      const { js } = compileModule(source, {
        filename: path,
        generate: "client"
      });
      return {
        contents: js.code,
        loader: "js",
      };
    });
  },
});

// Setup JSDOM environment
const dom = new JSDOM("<!DOCTYPE html><html><body></body></html>", {
  url: "http://localhost"
});

// Bind window and document to global scope
globalThis.window = dom.window as unknown as Window & typeof globalThis;
globalThis.document = dom.window.document;
globalThis.navigator = dom.window.navigator;

// Copy JSDOM classes and functions to global scope
Object.getOwnPropertyNames(dom.window).forEach((key) => {
  if (key in globalThis) return;
  // @ts-ignore
  globalThis[key] = dom.window[key];
});
