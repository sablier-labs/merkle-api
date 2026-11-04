import { chmodSync, copyFileSync, existsSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const endpoints = ["create", "eligibility", "health", "validity"];
const requiredKeys = [
  "IPFS_GATEWAY",
  "MERKLE_API_BEARER_TOKEN",
  "PINATA_ACCESS_TOKEN",
  "PINATA_API_KEY",
  "PINATA_API_SERVER",
  "PINATA_SECRET_API_KEY",
];

export function packageDotenvx(outputDirectory, envFile, dotenvxBinary) {
  const contents = readFileSync(envFile, "utf8");
  const keys = new Set();
  for (const line of contents.split(/\r?\n/)) {
    if (!line.trim() || line.startsWith("#")) continue;
    const match = line.match(/^([A-Z_]+)="([^"]+)"$/);
    if (!match) throw new Error("Invalid encrypted environment file format");
    const [, key, value] = match;
    if (keys.has(key)) throw new Error(`Duplicate environment variable: ${key}`);
    keys.add(key);
    if (key === "DOTENV_PUBLIC_KEY_PRODUCTION") {
      if (!/^[0-9a-f]{66}$/.test(value)) throw new Error("Invalid dotenvx public key");
    } else if (!requiredKeys.includes(key) || !/^encrypted:[A-Za-z0-9+/]+=*$/.test(value)) {
      throw new Error(`Unexpected or unencrypted environment variable: ${key}`);
    }
  }
  for (const key of ["DOTENV_PUBLIC_KEY_PRODUCTION", ...requiredKeys]) {
    if (!keys.has(key)) throw new Error(`Missing encrypted environment variable: ${key}`);
  }
  if (!existsSync(dotenvxBinary)) throw new Error("Missing dotenvx executable");

  const functions = endpoints.map((endpoint) => resolve(outputDirectory, "functions/api", `${endpoint}.func`));
  for (const directory of functions) {
    const config = JSON.parse(readFileSync(resolve(directory, ".vc-config.json"), "utf8"));
    if (config.runtime !== "executable" || config.handler !== "executable" || config.architecture !== "x86_64") {
      throw new Error(`Unexpected Rust runtime configuration: ${directory}`);
    }
    if (!existsSync(resolve(directory, "executable")) || existsSync(resolve(directory, "merkle-api"))) {
      throw new Error(`Missing or already wrapped Rust executable: ${directory}`);
    }
  }

  const bootstrap = readFileSync(new URL("./bootstrap.sh", import.meta.url));
  for (const directory of functions) {
    renameSync(resolve(directory, "executable"), resolve(directory, "merkle-api"));
    copyFileSync(dotenvxBinary, resolve(directory, "dotenvx"));
    chmodSync(resolve(directory, "dotenvx"), 0o755);
    copyFileSync(envFile, resolve(directory, ".env.production"));
    chmodSync(resolve(directory, ".env.production"), 0o644);
    writeFileSync(resolve(directory, "executable"), bootstrap, { mode: 0o755 });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const binary = process.argv[2];
  if (!binary) throw new Error("Usage: node scripts/package-dotenvx.mjs <dotenvx-binary>");
  packageDotenvx(".vercel/output", ".env.production", binary);
  console.log("Packaged dotenvx and encrypted configuration in all four Rust functions.");
}
