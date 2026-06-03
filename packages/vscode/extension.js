const cp = require("child_process");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");

let outputChannel;
let resultPanel;

function activate(context) {
  outputChannel = vscode.window.createOutputChannel("MarkApiDown");

  context.subscriptions.push(
    outputChannel,
    vscode.commands.registerCommand("markapidown.previewEndpoint", previewEndpoint),
    vscode.commands.registerCommand("markapidown.runEndpoint", runEndpoint),
    vscode.commands.registerCommand("markapidown.validateFile", validateFile),
    vscode.commands.registerCommand("markapidown.showContext", showContext),
    vscode.languages.registerCompletionItemProvider(
      [{ language: "markdown", scheme: "file" }],
      new VariableCompletionProvider(),
      "{",
      ":"
    )
  );
}

function deactivate() {}

async function previewEndpoint(uri) {
  const doc = await resolveDocument(uri);
  if (!doc) return;
  const file = doc.uri.fsPath;
  const root = await findApiDocsRoot(file);
  const variables = await collectVariables(file);

  const panel = vscode.window.createWebviewPanel(
    "markapidown.preview",
    `MarkApiDown Preview: ${path.basename(file)}`,
    vscode.ViewColumn.Beside,
    { enableCommandUris: true }
  );

  const runLink = commandUri("markapidown.runEndpoint", file);
  const validateLink = commandUri("markapidown.validateFile", file);
  const contextLink = commandUri("markapidown.showContext", file);
  panel.webview.html = renderPreviewHtml({
    file,
    root,
    source: doc.getText(),
    variables,
    runLink,
    validateLink,
    contextLink,
  });
}

async function runEndpoint(uri) {
  const doc = await resolveDocument(uri);
  if (!doc) return;
  if (!(await ensureSaved(doc))) return;
  const file = doc.uri.fsPath;
  const root = await findApiDocsRoot(file);
  const env = config().get("env", "dev");

  const result = await withProgress(`Running ${path.basename(file)}`, () =>
    runMad(["exec", file, "--env", env, "--output", "json"], root)
  );

  outputResult(result);
  showCommandResult("Run endpoint", file, result, "run");
  if (result.code === 0) {
    vscode.window.showInformationMessage("MarkApiDown endpoint run completed.");
  } else {
    vscode.window.showWarningMessage("MarkApiDown endpoint run failed. See result panel.");
  }
}

async function validateFile(uri) {
  const doc = await resolveDocument(uri);
  if (!doc) return;
  if (!(await ensureSaved(doc))) return;
  const file = doc.uri.fsPath;
  const root = await findApiDocsRoot(file);

  const result = await withProgress(`Validating ${path.basename(file)}`, () =>
    runMad(["validate", file], root)
  );

  outputResult(result);
  showCommandResult("Validate file", file, result, "validate");
  if (result.code === 0) {
    vscode.window.showInformationMessage("MarkApiDown validation passed.");
  } else {
    vscode.window.showWarningMessage("MarkApiDown validation failed. See result panel.");
  }
}

async function showContext(uri) {
  const doc = await resolveDocument(uri);
  if (!doc) return;
  if (!(await ensureSaved(doc))) return;
  const file = doc.uri.fsPath;
  const root = await findApiDocsRoot(file);
  const env = config().get("env", "dev");

  const result = await withProgress(`Loading context for ${path.basename(file)}`, () =>
    runMad(["context", file, "--root", root, "--env", env], root)
  );

  outputResult(result);
  showCommandResult("Agent context", file, result, "context");
}

class VariableCompletionProvider {
  async provideCompletionItems(document, position) {
    if (!document.uri.fsPath || !(await isInsideApiDocs(document.uri.fsPath))) {
      return [];
    }

    const prefix = document.lineAt(position).text.slice(0, position.character);
    const templateStart = prefix.lastIndexOf("{{");
    const templateEnd = prefix.lastIndexOf("}}");
    const inTemplate = templateStart > templateEnd;
    const afterPathColon = /(?:path:\s*.*|https?:\/\/.*|{{baseUrl}}.*):[A-Za-z0-9_]*$/.test(prefix);

    if (!inTemplate && !afterPathColon) {
      return [];
    }

    const variables = await collectVariables(document.uri.fsPath);
    return variables.map((entry) => {
      const item = new vscode.CompletionItem(entry.name, vscode.CompletionItemKind.Variable);
      item.detail = `MarkApiDown ${entry.sources.join(", ")}`;
      item.documentation = new vscode.MarkdownString(
        `Variable from ${entry.sources.map((s) => `\`${s}\``).join(", ")}.`
      );
      item.insertText = entry.name;
      return item;
    });
  }
}

async function resolveDocument(uri) {
  if (typeof uri === "string") {
    return vscode.workspace.openTextDocument(vscode.Uri.file(uri));
  }
  if (uri && uri.fsPath) {
    return vscode.workspace.openTextDocument(uri);
  }
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    vscode.window.showWarningMessage("Open a MarkApiDown markdown file first.");
    return undefined;
  }
  if (editor.document.languageId !== "markdown") {
    vscode.window.showWarningMessage("MarkApiDown commands run on markdown spec files.");
    return undefined;
  }
  return editor.document;
}

async function ensureSaved(doc) {
  if (!doc.isDirty) return true;
  const saved = await doc.save();
  if (!saved) {
    vscode.window.showWarningMessage("Save the current file before running MarkApiDown.");
  }
  return saved;
}

function config() {
  return vscode.workspace.getConfiguration("markapidown");
}

async function runMad(args, apiDocsRoot) {
  const madPath = config().get("madPath", "mad");
  const cwd = projectRootFor(apiDocsRoot);
  return new Promise((resolve) => {
    cp.execFile(
      madPath,
      args,
      {
        cwd,
        timeout: 120000,
        maxBuffer: 10 * 1024 * 1024,
        env: process.env,
      },
      (error, stdout, stderr) => {
        resolve({
          command: `${madPath} ${args.map(shellQuote).join(" ")}`,
          code: error ? (typeof error.code === "number" ? error.code : 127) : 0,
          stdout: stdout || "",
          stderr: stderr || "",
          error: error && !stdout && !stderr ? error.message : "",
          cwd,
        });
      }
    );
  });
}

function withProgress(title, task) {
  return vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title,
      cancellable: false,
    },
    task
  );
}

function outputResult(result) {
  outputChannel.appendLine(`$ ${result.command}`);
  if (result.cwd) outputChannel.appendLine(`cwd: ${result.cwd}`);
  if (result.stdout) outputChannel.appendLine(result.stdout.trimEnd());
  if (result.stderr) outputChannel.appendLine(result.stderr.trimEnd());
  if (result.error) outputChannel.appendLine(result.error);
  outputChannel.appendLine(`exit: ${result.code}`);
  outputChannel.appendLine("");
}

function showCommandResult(title, file, result, kind) {
  if (!config().get("resultPanel", true)) {
    outputChannel.show(true);
    return;
  }

  if (!resultPanel) {
    resultPanel = vscode.window.createWebviewPanel(
      "markapidown.results",
      "MarkApiDown Results",
      vscode.ViewColumn.Beside,
      {}
    );
    resultPanel.onDidDispose(() => {
      resultPanel = undefined;
    });
  }

  resultPanel.title = `MarkApiDown: ${title}`;
  resultPanel.webview.html = renderResultHtml({ title, file, result, kind });
  resultPanel.reveal(vscode.ViewColumn.Beside);
}

async function collectVariables(file) {
  const root = await findApiDocsRoot(file);
  const variables = new Map();
  const add = (name, source) => {
    if (!name || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) return;
    if (!variables.has(name)) variables.set(name, new Set());
    variables.get(name).add(source);
  };

  add("baseUrl", "default");
  for (const name of await envMdKeys(root)) add(name, "_shared/env.md");
  for (const name of await dotenvKeys(root)) add(name, ".env.local");
  for (const name of await flowCaptureKeys(root)) add(name, "flow capture");
  for (const name of await currentFileKeys(file)) add(name, "current spec");

  return [...variables.entries()]
    .map(([name, sources]) => ({ name, sources: [...sources].sort() }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

async function envMdKeys(root) {
  const source = await readOptional(path.join(root, "_shared", "env.md"));
  const keys = new Set();
  let inYaml = false;
  for (const line of source.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed === "```yaml" || trimmed === "```yml") {
      inYaml = true;
      continue;
    }
    if (inYaml && trimmed === "```") {
      inYaml = false;
      continue;
    }
    if (!inYaml) continue;
    const match = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)\s*:/);
    if (match) keys.add(match[1]);
  }
  return keys;
}

async function dotenvKeys(root) {
  const source = await readOptional(path.join(projectRootFor(root), ".env.local"));
  const keys = new Set();
  for (const line of source.split(/\r?\n/)) {
    const match = line.trim().match(/^([A-Za-z_][A-Za-z0-9_]*)\s*=/);
    if (match) {
      keys.add(match[1]);
      if (/^[A-Z0-9_]+$/.test(match[1])) keys.add(envNameToVar(match[1]));
    }
  }
  return keys;
}

async function flowCaptureKeys(root) {
  const keys = new Set();
  for (const dir of [path.join(root, "flows"), path.join(root, "pipelines")]) {
    for (const file of await markdownFiles(dir)) {
      const source = await readOptional(file);
      const captureRe = /Capture:\s*`?[^`\s]+`?\s+as\s+`?([A-Za-z_][A-Za-z0-9_]*)`?/g;
      let match;
      while ((match = captureRe.exec(source))) {
        keys.add(match[1]);
      }
    }
  }
  return keys;
}

async function currentFileKeys(file) {
  const source = await readOptional(file);
  const keys = new Set();
  const templateRe = /\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}/g;
  const pathParamRe = /:([A-Za-z_][A-Za-z0-9_]*)/g;
  let match;
  while ((match = templateRe.exec(source))) keys.add(match[1]);
  while ((match = pathParamRe.exec(source))) keys.add(match[1]);
  return keys;
}

async function findApiDocsRoot(file) {
  const configured = config().get("apiDocsRoot", "");
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (configured) {
    const configuredPath = path.isAbsolute(configured)
      ? configured
      : path.join(workspaceFolder || path.dirname(file), configured);
    if (await exists(configuredPath)) return configuredPath;
  }

  let current = fs.statSync(file).isDirectory() ? file : path.dirname(file);
  while (true) {
    if (path.basename(current) === "api-docs" && (await exists(path.join(current, "mad.md")))) {
      return current;
    }
    if (await exists(path.join(current, "api-docs", "mad.md"))) {
      return path.join(current, "api-docs");
    }
    const parent = path.dirname(current);
    if (parent === current) break;
    current = parent;
  }

  if (workspaceFolder && (await exists(path.join(workspaceFolder, "api-docs", "mad.md")))) {
    return path.join(workspaceFolder, "api-docs");
  }

  return workspaceFolder ? path.join(workspaceFolder, "api-docs") : path.dirname(file);
}

async function isInsideApiDocs(file) {
  const root = await findApiDocsRoot(file);
  const rel = path.relative(root, file);
  return rel && !rel.startsWith("..") && !path.isAbsolute(rel);
}

function projectRootFor(apiDocsRoot) {
  return path.basename(apiDocsRoot) === "api-docs" ? path.dirname(apiDocsRoot) : apiDocsRoot;
}

async function markdownFiles(dir) {
  if (!(await exists(dir))) return [];
  const out = [];
  const entries = await fs.promises.readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...(await markdownFiles(full)));
    } else if (entry.isFile() && entry.name.endsWith(".md")) {
      out.push(full);
    }
  }
  return out.sort();
}

async function readOptional(file) {
  try {
    return await fs.promises.readFile(file, "utf8");
  } catch {
    return "";
  }
}

async function exists(file) {
  try {
    await fs.promises.access(file);
    return true;
  } catch {
    return false;
  }
}

function envNameToVar(name) {
  const lower = name.toLowerCase();
  return lower.replace(/_([a-z0-9])/g, (_, ch) => ch.toUpperCase());
}

function renderPreviewHtml({ file, root, source, variables, runLink, validateLink, contextLink }) {
  const variableBadges = variables.length
    ? variables
        .map((v) => `<span class="badge">${escapeHtml(v.name)} <small>${escapeHtml(v.sources.join(", "))}</small></span>`)
        .join("")
    : `<span class="muted">No variables found.</span>`;

  return htmlPage(
    "MarkApiDown Preview",
    `
    <header>
      <div>
        <h1>${escapeHtml(path.basename(file))}</h1>
        <p>${escapeHtml(path.relative(projectRootFor(root), file))}</p>
      </div>
      <nav>
        <a class="button" href="${runLink}">Run</a>
        <a class="button" href="${validateLink}">Validate</a>
        <a class="button" href="${contextLink}">Context</a>
      </nav>
    </header>
    <section class="panel">
      <h2>Variables</h2>
      <div class="badges">${variableBadges}</div>
    </section>
    <main class="markdown">${renderMarkdown(source)}</main>
    `
  );
}

function renderResultHtml({ title, file, result, kind }) {
  const parsed = parseJson(result.stdout);
  let summary = "";
  if (kind === "run" && parsed) {
    const status = parsed.response?.status || parsed.status || "n/a";
    const passed = parsed.diff?.passed === true || parsed.passed === true;
    const assertions = Array.isArray(parsed.assertion_results)
      ? parsed.assertion_results
      : [];
    summary = `
      <section class="panel">
        <h2>${passed ? "Passed" : "Failed"}</h2>
        <dl>
          <dt>Status</dt><dd>${escapeHtml(String(status))}</dd>
          <dt>Duration</dt><dd>${escapeHtml(String(parsed.duration_ms || 0))} ms</dd>
          <dt>Request</dt><dd>${escapeHtml([parsed.request?.method, parsed.request?.url].filter(Boolean).join(" "))}</dd>
        </dl>
        ${renderDiff(parsed.diff)}
        ${renderAssertions(assertions)}
      </section>`;
  } else if (kind === "context") {
    summary = `<section class="panel"><h2>Context</h2><pre>${escapeHtml(result.stdout || result.stderr || result.error)}</pre></section>`;
  } else {
    summary = `<section class="panel"><h2>${result.code === 0 ? "Passed" : "Failed"}</h2><pre>${escapeHtml(result.stdout || result.stderr || result.error)}</pre></section>`;
  }

  return htmlPage(
    `MarkApiDown ${title}`,
    `
    <header>
      <div>
        <h1>${escapeHtml(title)}</h1>
        <p>${escapeHtml(file)}</p>
      </div>
      <span class="${result.code === 0 ? "ok" : "fail"}">exit ${result.code}</span>
    </header>
    ${summary}
    <section class="panel">
      <h2>Command</h2>
      <pre>${escapeHtml(result.command)}</pre>
    </section>
    <section class="panel">
      <h2>Raw output</h2>
      <pre>${escapeHtml([result.stdout, result.stderr, result.error].filter(Boolean).join("\n"))}</pre>
    </section>
    `
  );
}

function renderDiff(diff) {
  if (!diff || diff.passed) return "";
  const rows = [];
  if (diff.status) rows.push(`<li>Status: ${escapeHtml(diff.status)}</li>`);
  for (const header of diff.headers || []) rows.push(`<li>Header: ${escapeHtml(header)}</li>`);
  if (diff.body) rows.push(`<li>Body: ${escapeHtml(diff.body)}</li>`);
  for (const assertion of diff.assertions || []) rows.push(`<li>Assertion: ${escapeHtml(assertion)}</li>`);
  return rows.length ? `<h3>Diff</h3><ul>${rows.join("")}</ul>` : "";
}

function renderAssertions(assertions) {
  if (!assertions.length) return "";
  return `
    <h3>Assertions</h3>
    <table>
      <thead><tr><th>Rule</th><th>Passed</th><th>Message</th></tr></thead>
      <tbody>
        ${assertions
          .map(
            (a) =>
              `<tr><td>${escapeHtml(a.rule || "")}</td><td>${a.passed ? "yes" : "no"}</td><td>${escapeHtml(a.message || "")}</td></tr>`
          )
          .join("")}
      </tbody>
    </table>`;
}

function renderMarkdown(source) {
  const lines = source.split(/\r?\n/);
  let html = "";
  let inCode = false;
  let code = [];
  let codeLang = "";
  let inFrontmatter = lines[0] === "---";
  let frontmatter = [];

  for (let i = inFrontmatter ? 1 : 0; i < lines.length; i++) {
    const line = lines[i];
    if (inFrontmatter) {
      if (line === "---") {
        html += `<section class="frontmatter"><h2>Frontmatter</h2><pre>${escapeHtml(frontmatter.join("\n"))}</pre></section>`;
        inFrontmatter = false;
      } else {
        frontmatter.push(line);
      }
      continue;
    }

    const fence = line.match(/^```(.*)$/);
    if (fence && !inCode) {
      inCode = true;
      codeLang = fence[1].trim();
      code = [];
      continue;
    }
    if (line.trim() === "```" && inCode) {
      html += `<pre class="code"><code data-lang="${escapeHtml(codeLang)}">${escapeHtml(code.join("\n"))}</code></pre>`;
      inCode = false;
      continue;
    }
    if (inCode) {
      code.push(line);
      continue;
    }

    if (line.startsWith("# ")) html += `<h1>${escapeHtml(line.slice(2))}</h1>`;
    else if (line.startsWith("## ")) html += `<h2>${escapeHtml(line.slice(3))}</h2>`;
    else if (line.startsWith("### ")) html += `<h3>${escapeHtml(line.slice(4))}</h3>`;
    else if (line.startsWith("- ")) html += `<li>${escapeHtml(line.slice(2))}</li>`;
    else if (line.trim() === "") html += "";
    else html += `<p>${escapeHtml(line)}</p>`;
  }
  return html;
}

function htmlPage(title, body) {
  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>${escapeHtml(title)}</title>
  <style>
    body { font-family: var(--vscode-font-family); color: var(--vscode-foreground); background: var(--vscode-editor-background); margin: 0; padding: 24px; }
    header { display: flex; justify-content: space-between; gap: 16px; align-items: flex-start; border-bottom: 1px solid var(--vscode-panel-border); padding-bottom: 16px; margin-bottom: 20px; }
    h1 { font-size: 22px; margin: 0 0 6px; }
    h2 { font-size: 16px; margin: 0 0 10px; }
    h3 { font-size: 14px; margin: 16px 0 8px; }
    p { line-height: 1.5; }
    nav { display: flex; gap: 8px; flex-wrap: wrap; }
    .button { color: var(--vscode-button-foreground); background: var(--vscode-button-background); text-decoration: none; padding: 6px 10px; border-radius: 4px; }
    .panel, .frontmatter { border: 1px solid var(--vscode-panel-border); padding: 14px; margin: 14px 0; border-radius: 6px; }
    .badge { display: inline-block; border: 1px solid var(--vscode-panel-border); border-radius: 999px; padding: 3px 8px; margin: 0 6px 6px 0; }
    .badge small { opacity: .7; }
    .muted { opacity: .75; }
    .ok { color: var(--vscode-testing-iconPassed); }
    .fail { color: var(--vscode-testing-iconFailed); }
    pre { overflow: auto; padding: 12px; background: var(--vscode-textCodeBlock-background); border-radius: 6px; }
    code::before { content: attr(data-lang); display: block; opacity: .65; margin-bottom: 8px; }
    table { width: 100%; border-collapse: collapse; }
    th, td { text-align: left; border-bottom: 1px solid var(--vscode-panel-border); padding: 6px; }
    dl { display: grid; grid-template-columns: max-content 1fr; gap: 6px 12px; }
    dt { opacity: .7; }
    dd { margin: 0; }
  </style>
</head>
<body>${body}</body>
</html>`;
}

function parseJson(stdout) {
  try {
    return JSON.parse(stdout.trim());
  } catch {
    return undefined;
  }
}

function commandUri(command, file) {
  return `command:${command}?${encodeURIComponent(JSON.stringify([file]))}`;
}

function shellQuote(value) {
  return /\s/.test(value) ? JSON.stringify(value) : value;
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

module.exports = {
  activate,
  deactivate,
};
