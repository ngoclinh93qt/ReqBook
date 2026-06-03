const path = require("path");
const os = require("os");

function detectReqbookSpec({ filePath, source, apiDocsRoot }) {
  if (!filePath || !apiDocsRoot || typeof source !== "string") return undefined;
  if (path.extname(filePath).toLowerCase() !== ".md") return undefined;

  const relPath = relativeInside(apiDocsRoot, filePath);
  if (!relPath) return undefined;

  const relParts = splitPath(relPath);
  const basename = path.basename(filePath).toLowerCase();
  if (basename === "reqbook.md" || basename === "mad.md" || basename === "readme.md") return undefined;
  if (relParts.includes("_shared")) return undefined;

  const frontmatter = parseFrontmatter(source);
  if (!frontmatter) return undefined;

  if (isFlowPath(relParts)) {
    return {
      kind: "flow",
      relPath,
    };
  }

  if (frontmatter.data.method && frontmatter.data.path) {
    return {
      kind: "endpoint",
      relPath,
    };
  }

  return undefined;
}

function relativeInside(root, file) {
  const rel = path.relative(path.resolve(root), path.resolve(file));
  if (!rel || rel.startsWith("..") || path.isAbsolute(rel)) return undefined;
  return rel;
}

function isFlowPath(parts) {
  return parts[0] === "flows" || parts[0] === "pipelines";
}

function parseFrontmatter(source) {
  const lines = source.replace(/^\uFEFF/, "").split(/\r?\n/);
  if (!lines.length || lines[0].trim() !== "---") return undefined;

  const data = {};
  const raw = [];
  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    if (line.trim() === "---") {
      return {
        data,
        raw: raw.join("\n"),
      };
    }
    raw.push(line);
    const match = line.match(/^\s*([A-Za-z][A-Za-z0-9_.-]*)\s*:\s*(.*?)\s*$/);
    if (match) {
      data[match[1].toLowerCase()] = stripQuotes(match[2]);
    }
  }

  return undefined;
}

function stripQuotes(value) {
  const trimmed = value.trim();
  const quoted = trimmed.match(/^(['"])(.*)\1$/);
  return quoted ? quoted[2] : trimmed;
}

function splitPath(value) {
  return path.normalize(value).split(/[\\/]+/).filter(Boolean);
}

function candidateReqbookBinaries({ configuredPath = "rqb", cwd, workspaceFolders = [], env = process.env }) {
  if (configuredPath && configuredPath !== "rqb") {
    return [configuredPath];
  }

  const candidates = [];
  const add = (value) => {
    if (value && !candidates.includes(value)) candidates.push(value);
  };
  const addBuildDirs = (root) => {
    add(path.join(root, "target", "debug", executableName("rqb")));
    add(path.join(root, "target", "release", executableName("rqb")));
  };

  add(env.RQB_PATH);
  for (const root of [cwd, ...parentDirs(cwd), ...workspaceFolders]) {
    if (root) addBuildDirs(root);
  }
  add(path.join(os.homedir(), ".cargo", "bin", executableName("rqb")));
  add("/opt/homebrew/bin/rqb");
  add("/usr/local/bin/rqb");
  add("/usr/bin/rqb");

  for (const dir of String(env.PATH || "").split(path.delimiter)) {
    add(path.join(dir, executableName("rqb")));
  }

  add("rqb");
  return candidates;
}

function executableName(name) {
  return process.platform === "win32" ? `${name}.exe` : name;
}

function parentDirs(start) {
  const dirs = [];
  if (!start) return dirs;
  let current = path.resolve(start);
  while (true) {
    const parent = path.dirname(current);
    if (parent === current) return dirs;
    dirs.push(parent);
    current = parent;
  }
}

module.exports = {
  detectReqbookSpec,
  candidateReqbookBinaries,
  parseFrontmatter,
};
