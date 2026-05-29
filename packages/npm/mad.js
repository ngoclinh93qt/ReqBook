#!/usr/bin/env node
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const https = require("https");

const version = process.env.MAD_VERSION || require("./package.json").version;
const repo = process.env.MAD_REPO || "mark-api-down/mad";
const cacheRoot = process.env.MAD_CACHE_DIR || path.join(os.homedir(), ".cache", "mark-api-down");

function target() {
  const platform = os.platform();
  const arch = os.arch();
  const archPart = arch === "arm64" ? "aarch64" : "x86_64";
  if (platform === "darwin") return `${archPart}-apple-darwin`;
  if (platform === "linux") return `${archPart}-unknown-linux-musl`;
  if (platform === "win32") return `${archPart}-pc-windows-msvc`;
  throw new Error(`unsupported platform: ${platform}/${arch}`);
}

function extension() {
  return os.platform() === "win32" ? ".zip" : ".tar.xz";
}

function binaryName() {
  return os.platform() === "win32" ? "mad.exe" : "mad";
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);
    https.get(url, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        file.close();
        fs.unlinkSync(dest);
        download(res.headers.location, dest).then(resolve, reject);
        return;
      }
      if (res.statusCode !== 200) {
        reject(new Error(`download failed ${res.statusCode}: ${url}`));
        return;
      }
      res.pipe(file);
      file.on("finish", () => file.close(resolve));
    }).on("error", reject);
  });
}

async function ensureBinary() {
  if (process.env.MAD_BINARY) {
    return process.env.MAD_BINARY;
  }
  const triple = target();
  const dir = path.join(cacheRoot, version, triple);
  const bin = path.join(dir, binaryName());
  if (fs.existsSync(bin)) return bin;

  fs.mkdirSync(dir, { recursive: true });
  const archive = `mad-${triple}${extension()}`;
  const archivePath = path.join(dir, archive);
  const url = `https://github.com/${repo}/releases/download/v${version}/${archive}`;
  await download(url, archivePath);

  const unpack = os.platform() === "win32"
    ? spawnSync("powershell", ["-NoProfile", "-Command", `Expand-Archive -Force ${JSON.stringify(archivePath)} ${JSON.stringify(dir)}`], { stdio: "inherit" })
    : spawnSync("tar", ["-xJf", archivePath, "-C", dir], { stdio: "inherit" });
  if (unpack.status !== 0) process.exit(unpack.status || 1);

  const found = findBinary(dir);
  if (!found) throw new Error(`archive did not contain ${binaryName()}`);
  fs.copyFileSync(found, bin);
  fs.chmodSync(bin, 0o755);
  return bin;
}

function findBinary(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      const nested = findBinary(full);
      if (nested) return nested;
    } else if (entry.name === binaryName()) {
      return full;
    }
  }
  return null;
}

ensureBinary()
  .then((bin) => {
    const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
    process.exit(result.status || 0);
  })
  .catch((err) => {
    console.error(err.message);
    process.exit(1);
  });
