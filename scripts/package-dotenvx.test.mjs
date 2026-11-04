import assert from "node:assert/strict";
import { chmodSync, existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { after, before, test } from "node:test";
import { packageDotenvx } from "./package-dotenvx.mjs";

const binary = process.env.DOTENVX_BINARY;
assert.ok(binary, "Set DOTENVX_BINARY to the official dotenvx executable");
const keys = [
  "IPFS_GATEWAY",
  "MERKLE_API_BEARER_TOKEN",
  "PINATA_ACCESS_TOKEN",
  "PINATA_API_KEY",
  "PINATA_API_SERVER",
  "PINATA_SECRET_API_KEY",
];
let scratch;
let encrypted;
let privateKey;

before(() => {
  scratch = mkdtempSync(join(tmpdir(), "merkle-dotenvx-test-"));
  const envFile = join(scratch, ".env.production");
  writeFileSync(envFile, keys.map((key) => `${key}="fixture-${key}"`).join("\n") + "\n");
  const result = spawnSync(
    binary,
    ["encrypt", "--quiet", "--no-armor", "--no-native", "-f", envFile, "-fk", join(scratch, ".env.keys")],
    { cwd: scratch },
  );
  assert.equal(result.status, 0, "Fixture encryption must succeed");
  encrypted = readFileSync(envFile, "utf8");
  privateKey = readFileSync(join(scratch, ".env.keys"), "utf8").match(
    /^DOTENV_PRIVATE_KEY_PRODUCTION="?([0-9a-f]+)"?$/m,
  )[1];
});
after(() => rmSync(scratch, { recursive: true, force: true }));

function fixture(contents = encrypted) {
  const root = mkdtempSync(join(scratch, "output-"));
  const envFile = join(root, ".env.production");
  writeFileSync(envFile, contents);
  for (const endpoint of ["create", "eligibility", "health", "validity"]) {
    const directory = join(root, "functions/api", `${endpoint}.func`);
    mkdirSync(directory, { recursive: true });
    writeFileSync(
      join(directory, ".vc-config.json"),
      JSON.stringify({
        runtime: "executable",
        handler: "executable",
        architecture: "x86_64",
        supportsResponseStreaming: true,
      }),
    );
    writeFileSync(
      join(directory, "executable"),
      "#!/bin/sh\nset -eu\n" +
        keys.map((key) => `test "$${key}" = "fixture-${key}"`).join("\n") +
        "\nprintf 'started'\n",
    );
    chmodSync(join(directory, "executable"), 0o755);
  }
  return { root, envFile, directory: join(root, "functions/api/health.func") };
}

function boot(directory, key = privateKey) {
  return spawnSync(join(directory, "executable"), [], {
    cwd: tmpdir(),
    encoding: "utf8",
    env: {
      PATH: process.env.PATH,
      DOTENV_PRIVATE_KEY_PRODUCTION: key,
      ...Object.fromEntries(keys.map((name) => [name, "stale"])),
    },
  });
}

test("all Lambdas decrypt before startup, override legacy values, and contain no private key", () => {
  const { root, envFile } = fixture();
  packageDotenvx(root, envFile, binary);
  for (const endpoint of ["create", "eligibility", "health", "validity"]) {
    const directory = join(root, "functions/api", `${endpoint}.func`);
    const result = boot(directory);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout, "started");
    assert.equal(existsSync(join(directory, ".env.keys")), false);
    assert.equal(readFileSync(join(directory, ".env.production"), "utf8"), encrypted);
  }
  assert.throws(() => packageDotenvx(root, envFile, binary), /already wrapped/);
});

test("missing key, wrong key, and corrupt ciphertext prevent application startup", () => {
  const { root, envFile, directory } = fixture();
  packageDotenvx(root, envFile, binary);
  for (const key of ["", "1".repeat(64)]) {
    const result = boot(directory, key);
    assert.notEqual(result.status, 0);
    assert.equal(result.stdout.includes("started"), false);
  }
  writeFileSync(join(directory, ".env.production"), encrypted.replace(/encrypted:[^"\n]+/, "encrypted:AAAA"));
  const result = boot(directory);
  assert.notEqual(result.status, 0);
  assert.equal(result.stdout.includes("started"), false);
});

test("packaging rejects plaintext, missing variables, and private keys before modifying functions", () => {
  for (const contents of [
    encrypted.replace(/PINATA_API_KEY="[^"]+"/, 'PINATA_API_KEY="plaintext"'),
    encrypted.replace(/^PINATA_API_KEY=.*\n/m, ""),
    `${encrypted}DOTENV_PRIVATE_KEY_PRODUCTION="${privateKey}"\n`,
  ]) {
    const { root, envFile, directory } = fixture(contents);
    assert.throws(() => packageDotenvx(root, envFile, binary), /environment variable/);
    assert.equal(existsSync(join(directory, "merkle-api")), false);
  }
});
