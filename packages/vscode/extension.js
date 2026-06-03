const cp = require("child_process");
const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const { candidateReqbookBinaries, detectReqbookSpec } = require("./detect");

let outputChannel;
let resultPanel;
let contextUpdateSeq = 0;

function activate(context) {
  outputChannel = vscode.window.createOutputChannel("Reqbook");
  const codeLensProvider = new ReqbookCodeLensProvider();

  context.subscriptions.push(
    outputChannel,
    codeLensProvider,
    vscode.languages.registerCodeLensProvider([{ language: "markdown", scheme: "file" }], codeLensProvider),
    vscode.commands.registerCommand("reqbook.previewEndpoint", previewEndpoint),
    vscode.commands.registerCommand("reqbook.runSpec", runSpec),
    vscode.commands.registerCommand("reqbook.runEndpoint", runEndpoint),
    vscode.commands.registerCommand("reqbook.validateFile", validateFile),
    vscode.commands.registerCommand("reqbook.showContext", showContext),
    vscode.languages.registerCompletionItemProvider(
      [{ language: "markdown", scheme: "file" }],
      new VariableCompletionProvider(),
      "{",
      ":"
    ),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      refreshRunnableContext(editor);
      codeLensProvider.refresh();
    }),
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (event.document.languageId !== "markdown") return;
      codeLensProvider.refresh();
      if (isActiveDocument(event.document)) {
        refreshRunnableContext(vscode.window.activeTextEditor);
      }
    }),
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (document.languageId !== "markdown") return;
      codeLensProvider.refresh();
      if (isActiveDocument(document)) {
        refreshRunnableContext(vscode.window.activeTextEditor);
      }
    })
  );

  refreshRunnableContext(vscode.window.activeTextEditor);
}

function deactivate() {}

async function previewEndpoint(uri) {
  const doc = await resolveDocument(uri);
  if (!doc) return;
  const file = doc.uri.fsPath;
  const root = await findApiDocsRoot(file);
  const spec = await detectRunnableDocument(doc);
  const variables = await collectVariables(file);

  const panel = vscode.window.createWebviewPanel(
    "reqbook.preview",
    `Reqbook Preview: ${path.basename(file)}`,
    vscode.ViewColumn.Beside,
    { enableCommandUris: true }
  );

  const runLink = spec ? commandUri("reqbook.runSpec", file) : undefined;
  const validateLink = commandUri("reqbook.validateFile", file);
  const contextLink = commandUri("reqbook.showContext", file);
  panel.webview.html = renderPreviewHtml({
    file,
    root,
    spec,
    source: doc.getText(),
    variables,
    runLink,
    validateLink,
    contextLink,
  });
}

async function runEndpoint(uri) {
  return runSpec(uri);
}

async function runSpec(uri) {
  const doc = await resolveDocument(uri);
  if (!doc) return;
  if (!(await ensureSaved(doc))) return;
  const file = doc.uri.fsPath;
  const spec = await detectRunnableDocument(doc);
  if (!spec) {
    vscode.window.showWarningMessage("Open a Reqbook endpoint or flow file first.");
    return;
  }

  const root = spec.root;
  const env = config().get("env", "dev");
  const isFlow = spec.kind === "flow";
  const title = isFlow ? "Run flow" : "Run endpoint";
  const args = isFlow
    ? ["flow", file, "--env", env, "--output", "json"]
    : ["exec", file, "--env", env, "--output", "json"];

  const result = await withProgress(`Running ${isFlow ? "flow" : "endpoint"} ${path.basename(file)}`, () =>
    runReqbook(args, root)
  );

  outputResult(result);
  showCommandResult(title, file, result, isFlow ? "flow" : "run");
  if (result.code === 0) {
    vscode.window.showInformationMessage(`Reqbook ${isFlow ? "flow" : "endpoint"} run completed.`);
  } else {
    vscode.window.showWarningMessage(`Reqbook ${isFlow ? "flow" : "endpoint"} run failed. See result panel.`);
  }
}

async function validateFile(uri) {
  const doc = await resolveDocument(uri);
  if (!doc) return;
  if (!(await ensureSaved(doc))) return;
  const file = doc.uri.fsPath;
  const root = await findApiDocsRoot(file);

  const result = await withProgress(`Validating ${path.basename(file)}`, () =>
    runReqbook(["validate", file], root)
  );

  outputResult(result);
  showCommandResult("Validate file", file, result, "validate");
  if (result.code === 0) {
    vscode.window.showInformationMessage("Reqbook validation passed.");
  } else {
    vscode.window.showWarningMessage("Reqbook validation failed. See result panel.");
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
    runReqbook(["context", file, "--root", root, "--env", env], root)
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
      item.detail = `Reqbook ${entry.sources.join(", ")}`;
      item.documentation = new vscode.MarkdownString(
        `Variable from ${entry.sources.map((s) => `\`${s}\``).join(", ")}.`
      );
      item.insertText = entry.name;
      return item;
    });
  }
}

class ReqbookCodeLensProvider {
  constructor() {
    this._onDidChangeCodeLenses = new vscode.EventEmitter();
    this.onDidChangeCodeLenses = this._onDidChangeCodeLenses.event;
  }

  refresh() {
    this._onDidChangeCodeLenses.fire();
  }

  dispose() {
    this._onDidChangeCodeLenses.dispose();
  }

  async provideCodeLenses(document) {
    const spec = await detectRunnableDocument(document);
    if (!spec) return [];

    const title = spec.kind === "flow" ? "$(play) Run Flow" : "$(play) Run Endpoint";
    return [
      new vscode.CodeLens(new vscode.Range(0, 0, 0, 0), {
        title,
        command: "reqbook.runSpec",
        arguments: [document.uri],
      }),
    ];
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
    vscode.window.showWarningMessage("Open a Reqbook markdown file first.");
    return undefined;
  }
  if (editor.document.languageId !== "markdown") {
    vscode.window.showWarningMessage("Reqbook commands run on markdown spec files.");
    return undefined;
  }
  return editor.document;
}

async function ensureSaved(doc) {
  if (!doc.isDirty) return true;
  const saved = await doc.save();
  if (!saved) {
    vscode.window.showWarningMessage("Save the current file before running Reqbook.");
  }
  return saved;
}

function config() {
  return vscode.workspace.getConfiguration("reqbook");
}

async function detectRunnableDocument(document) {
  if (!document || document.languageId !== "markdown" || document.uri.scheme !== "file" || !document.uri.fsPath) {
    return undefined;
  }

  const file = document.uri.fsPath;
  const root = await findApiDocsRoot(file);
  if (!(await hasReqbookManifest(root))) {
    return undefined;
  }

  const spec = detectReqbookSpec({
    filePath: file,
    source: document.getText(),
    apiDocsRoot: root,
  });

  return spec ? { ...spec, root } : undefined;
}

async function updateRunnableContext(editor) {
  const seq = ++contextUpdateSeq;
  const spec = editor ? await detectRunnableDocument(editor.document) : undefined;
  if (seq !== contextUpdateSeq) return;
  await vscode.commands.executeCommand("setContext", "reqbook.runnableSpec", Boolean(spec));
}

function refreshRunnableContext(editor) {
  updateRunnableContext(editor).catch((error) => {
    outputChannel?.appendLine(`Failed to update Reqbook editor context: ${error.message || error}`);
  });
}

function isActiveDocument(document) {
  return vscode.window.activeTextEditor?.document.uri.toString() === document.uri.toString();
}

async function runReqbook(args, apiDocsRoot) {
  const cwd = projectRootFor(apiDocsRoot);
  const rqbPath = await resolveReqbookBinary(cwd);
  return new Promise((resolve) => {
    cp.execFile(
      rqbPath,
      args,
      {
        cwd,
        timeout: 120000,
        maxBuffer: 10 * 1024 * 1024,
        env: process.env,
      },
      (error, stdout, stderr) => {
        resolve({
          command: `${rqbPath} ${args.map(shellQuote).join(" ")}`,
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

async function resolveReqbookBinary(cwd) {
  const configuredPath = config().get("rqbPath", "rqb");
  const workspaceFolders = (vscode.workspace.workspaceFolders || []).map((folder) => folder.uri.fsPath);
  const candidates = candidateReqbookBinaries({
    configuredPath,
    cwd,
    workspaceFolders,
    env: process.env,
  });

  for (const candidate of candidates) {
    if (candidate === "rqb") continue;
    if (await isExecutable(candidate)) {
      if (configuredPath === "rqb") {
        outputChannel?.appendLine(`Auto-detected rqb binary: ${candidate}`);
      }
      return candidate;
    }
  }

  return configuredPath || "rqb";
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
      "reqbook.results",
      "Reqbook Results",
      vscode.ViewColumn.Beside,
      {}
    );
    resultPanel.onDidDispose(() => {
      resultPanel = undefined;
    });
  }

  resultPanel.title = `Reqbook: ${title}`;
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

  let current;
  try {
    current = fs.statSync(file).isDirectory() ? file : path.dirname(file);
  } catch {
    current = path.dirname(file);
  }
  while (true) {
    if (await hasReqbookManifest(current)) {
      return current;
    }
    if (await hasReqbookManifest(path.join(current, "api-docs"))) {
      return path.join(current, "api-docs");
    }
    const parent = path.dirname(current);
    if (parent === current) break;
    current = parent;
  }

  if (workspaceFolder) {
    if (await hasReqbookManifest(workspaceFolder)) {
      return workspaceFolder;
    }
    if (await hasReqbookManifest(path.join(workspaceFolder, "api-docs"))) {
      return path.join(workspaceFolder, "api-docs");
    }
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

async function isExecutable(file) {
  try {
    await fs.promises.access(file, fs.constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

function envNameToVar(name) {
  const lower = name.toLowerCase();
  return lower.replace(/_([a-z0-9])/g, (_, ch) => ch.toUpperCase());
}

function renderPreviewHtml({ file, root, spec, source, variables, runLink, validateLink, contextLink }) {
  const variableBadges = variables.length
    ? variables
        .map((v) => `<span class="badge">${escapeHtml(v.name)} <small>${escapeHtml(v.sources.join(", "))}</small></span>`)
        .join("")
    : `<span class="muted">No variables found.</span>`;
  const runButton = runLink
    ? `<a class="button" href="${runLink}">${spec.kind === "flow" ? "Run Flow" : "Run Endpoint"}</a>`
    : "";

  return htmlPage(
    "Reqbook Preview",
    `
    <header>
      <div>
        <h1>${escapeHtml(path.basename(file))}</h1>
        <p>${escapeHtml(path.relative(projectRootFor(root), file))}</p>
      </div>
      <nav>
        ${runButton}
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
  } else if (kind === "flow" && parsed) {
    const steps = Array.isArray(parsed.steps) ? parsed.steps : [];
    summary = `
      <section class="panel">
        <h2>${parsed.passed ? "Passed" : "Failed"}</h2>
        <dl>
          <dt>Steps</dt><dd>${escapeHtml(String(steps.length))}</dd>
          <dt>Captures</dt><dd>${escapeHtml(String(Object.keys(parsed.captures || {}).length))}</dd>
        </dl>
        ${renderFlowSteps(steps)}
      </section>`;
  } else if (kind === "context") {
    summary = `<section class="panel"><h2>Context</h2><pre>${escapeHtml(result.stdout || result.stderr || result.error)}</pre></section>`;
  } else {
    summary = `<section class="panel"><h2>${result.code === 0 ? "Passed" : "Failed"}</h2><pre>${escapeHtml(result.stdout || result.stderr || result.error)}</pre></section>`;
  }

  return htmlPage(
    `Reqbook ${title}`,
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

function renderFlowSteps(steps) {
  if (!steps.length) return "";
  return `
    <h3>Steps</h3>
    <table>
      <thead><tr><th>Step</th><th>Endpoint</th><th>Passed</th><th>Status / Error</th></tr></thead>
      <tbody>
        ${steps
          .map((step) => {
            const execution = step.execution || {};
            const passed = step.error ? false : execution.diff?.passed !== false;
            const status = step.error || execution.response?.status || "";
            return `<tr><td>${escapeHtml(step.name || "")}</td><td>${escapeHtml(step.endpoint || "")}</td><td>${passed ? "yes" : "no"}</td><td>${escapeHtml(status)}</td></tr>`;
          })
          .join("")}
      </tbody>
    </table>`;
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

async function hasReqbookManifest(apiDocsRoot) {
  return (await exists(path.join(apiDocsRoot, "reqbook.md"))) || (await exists(path.join(apiDocsRoot, "mad.md")));
}

module.exports = {
  activate,
  deactivate,
};
