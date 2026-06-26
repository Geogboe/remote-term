import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { build } from "esbuild";

const root = path.resolve(import.meta.dirname, "..");
const outDir = path.join(root, "src", "web", "static");

await mkdir(outDir, { recursive: true });
await build({
  entryPoints: [path.join(import.meta.dirname, "src", "main.js")],
  bundle: true,
  format: "iife",
  sourcemap: false,
  minify: true,
  outfile: path.join(outDir, "main.js")
});

const xtermCss = await readFile(path.join(import.meta.dirname, "node_modules", "@xterm", "xterm", "css", "xterm.css"), "utf8");
const appCss = await readFile(path.join(import.meta.dirname, "src", "style.css"), "utf8");
await writeFile(path.join(outDir, "style.css"), `${xtermCss}\n${appCss}`);
await copyFile(path.join(import.meta.dirname, "src", "index.html"), path.join(outDir, "index.html"));
