const assert = require("node:assert/strict");
const path = require("node:path");
const test = require("node:test");

const { candidateReqbookBinaries, detectReqbookSpec } = require("../detect");

const root = path.join("/repo", "api-docs");

function spec(relPath, source) {
  return detectReqbookSpec({
    filePath: path.join(root, relPath),
    apiDocsRoot: root,
    source,
  });
}

test("detects endpoint specs with method and path frontmatter", () => {
  const detected = spec(
    path.join("apis", "users", "get-user.md"),
    `---
resource: users
method: GET
path: /users/:id
---

# Get user
`
  );

  assert.equal(detected.kind, "endpoint");
  assert.equal(detected.relPath, path.join("apis", "users", "get-user.md"));
});

test("detects flow specs under flows", () => {
  const detected = spec(
    path.join("flows", "signup.md"),
    `---
type: pipeline
name: signup
---

# Signup
`
  );

  assert.equal(detected.kind, "flow");
});

test("detects pipeline specs under pipelines", () => {
  const detected = spec(
    path.join("pipelines", "smoke.md"),
    `---
type: pipeline
name: smoke
---

# Smoke
`
  );

  assert.equal(detected.kind, "flow");
});

test("ignores non-runnable collection docs", () => {
  const source = `---
name: docs
---

# Docs
`;

  assert.equal(spec("reqbook.md", source), undefined);
  assert.equal(spec("README.md", source), undefined);
  assert.equal(spec(path.join("_shared", "env.md"), source), undefined);
});

test("detects specs under a custom collection root", () => {
  const customRoot = path.join("/repo", "docs", "http-specs");
  const detected = detectReqbookSpec({
    filePath: path.join(customRoot, "apis", "users", "get-user.md"),
    apiDocsRoot: customRoot,
    source: `---
method: GET
path: /users/:id
---
`,
  });

  assert.equal(detected.kind, "endpoint");
  assert.equal(detected.relPath, path.join("apis", "users", "get-user.md"));
});

test("ignores markdown outside the collection root", () => {
  const detected = detectReqbookSpec({
    filePath: path.join("/repo", "README.md"),
    apiDocsRoot: root,
    source: `---
method: GET
path: /users
---
`,
  });

  assert.equal(detected, undefined);
});

test("does not classify endpoints without method or path", () => {
  assert.equal(
    spec(
      path.join("apis", "users", "missing-method.md"),
      `---
resource: users
path: /users
---
`
    ),
    undefined
  );

  assert.equal(
    spec(
      path.join("apis", "users", "missing-path.md"),
      `---
resource: users
method: GET
---
`
    ),
    undefined
  );
});

test("does not treat nested folders named flows as top-level flow specs", () => {
  const detected = spec(
    path.join("apis", "flows", "get-flow.md"),
    `---
resource: flows
method: GET
path: /flows/:id
---
`
  );

  assert.equal(detected.kind, "endpoint");
});

test("builds rqb binary candidates from workspace and common install paths", () => {
  const candidates = candidateReqbookBinaries({
    configuredPath: "rqb",
    cwd: path.join("/repo", "examples", "jsonplaceholder"),
    workspaceFolders: ["/repo"],
    env: {
      RQB_PATH: "/custom/rqb",
      PATH: ["/opt/bin", "/usr/local/bin"].join(path.delimiter),
    },
  });

  assert.equal(candidates[0], "/custom/rqb");
  assert.ok(candidates.includes(path.join("/repo", "target", "debug", "rqb")));
  assert.ok(candidates.includes(path.join("/repo", "target", "release", "rqb")));
  assert.ok(candidates.includes(path.join("/repo", "examples", "jsonplaceholder", "target", "debug", "rqb")));
  assert.ok(candidates.includes(path.join("/repo", "examples", "target", "debug", "rqb")));
  assert.ok(candidates.includes("/opt/bin/rqb"));
  assert.equal(candidates.at(-1), "rqb");
});

test("uses explicit configured rqb path as the only candidate", () => {
  assert.deepEqual(
    candidateReqbookBinaries({
      configuredPath: "/explicit/rqb",
      cwd: "/repo",
      workspaceFolders: ["/repo"],
      env: {},
    }),
    ["/explicit/rqb"]
  );
});
