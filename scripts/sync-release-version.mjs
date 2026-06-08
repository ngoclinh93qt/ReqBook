#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const rawVersion = process.argv[2];
if (!rawVersion) {
  console.error('usage: node scripts/sync-release-version.mjs <version-or-vtag>');
  process.exit(2);
}

const version = rawVersion.startsWith('v') ? rawVersion.slice(1) : rawVersion;
if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`invalid release version: ${rawVersion}`);
  process.exit(2);
}

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const changed = [];

function filePath(relativePath) {
  return path.join(root, relativePath);
}

function read(relativePath) {
  return fs.readFileSync(filePath(relativePath), 'utf8');
}

function write(relativePath, content) {
  const absolutePath = filePath(relativePath);
  const previous = fs.existsSync(absolutePath) ? fs.readFileSync(absolutePath, 'utf8') : '';
  if (previous !== content) {
    fs.writeFileSync(absolutePath, content);
    if (!changed.includes(relativePath)) {
      changed.push(relativePath);
    }
  }
}

function replaceInFile(relativePath, pattern, replacement) {
  const content = read(relativePath);
  const found =
    typeof pattern === 'string'
      ? content.includes(pattern)
      : new RegExp(pattern.source, pattern.flags.replace('g', '')).test(content);
  if (!found) {
    throw new Error(`pattern not found in ${relativePath}: ${pattern}`);
  }
  const next = content.replace(pattern, replacement);
  write(relativePath, next);
}

function updateJson(relativePath, update) {
  const json = JSON.parse(read(relativePath));
  update(json);
  write(relativePath, `${JSON.stringify(json, null, 2)}\n`);
}

function updateCargoToml(relativePath) {
  replaceInFile(
    relativePath,
    /(\[package\][\s\S]*?\nversion\s*=\s*)"[^"]+"/,
    `$1"${version}"`,
  );
}

function updateCargoLockPackage(packageName) {
  const escapedName = packageName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  replaceInFile(
    'Cargo.lock',
    new RegExp(`(\\[\\[package\\]\\]\\nname = "${escapedName}"\\nversion = )"[^"]+"`),
    `$1"${version}"`,
  );
}

function updateVsixNames(relativePath) {
  replaceInFile(
    relativePath,
    /reqbook-vscode-\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?\.vsix/g,
    `reqbook-vscode-${version}.vsix`,
  );
}

updateCargoToml('Cargo.toml');
updateCargoToml('src-tauri/Cargo.toml');
updateCargoLockPackage('reqbook');
updateCargoLockPackage('rqb-desktop');

updateJson('src-tauri/tauri.conf.json', (json) => {
  json.version = version;
});
updateJson('packages/npm/package.json', (json) => {
  json.version = version;
});
updateJson('packages/vscode/package.json', (json) => {
  json.version = version;
});
updateJson('packages/vscode/package-lock.json', (json) => {
  json.version = version;
  if (json.packages?.['']) {
    json.packages[''].version = version;
  }
});

replaceInFile(
  'docs/installation.mdx',
  /ghcr\.io\/ngoclinh93qt\/rqb:\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?/g,
  `ghcr.io/ngoclinh93qt/rqb:${version}`,
);
replaceInFile(
  'docs/installation.mdx',
  /rqb version\n# \d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?/,
  `rqb version\n# ${version}`,
);

updateVsixNames('packages/vscode/README.md');
updateVsixNames('packages/vscode/PUBLISHING.md');

const changelogPath = 'packages/vscode/CHANGELOG.md';
const changelog = read(changelogPath);
if (!new RegExp(`^## ${version.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')} - `, 'm').test(changelog)) {
  const today = new Date().toISOString().slice(0, 10);
  write(
    changelogPath,
    changelog.replace(
      /^# Changelog\n\n/,
      `# Changelog\n\n## ${version} - ${today}\n\n- Release artifacts are versioned from the Git tag.\n\n`,
    ),
  );
}

console.log(`release version: ${version}`);
if (changed.length > 0) {
  console.log('updated files:');
  for (const relativePath of changed) {
    console.log(`- ${relativePath}`);
  }
} else {
  console.log('all release-facing files already matched the tag');
}
