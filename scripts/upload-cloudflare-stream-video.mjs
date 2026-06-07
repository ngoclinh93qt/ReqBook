#!/usr/bin/env node
import { basename, resolve } from 'node:path';
import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';

const accountId = process.env.CLOUDFLARE_ACCOUNT_ID || process.env.CF_ACCOUNT_ID;
const token =
  process.env.CLOUDFLARE_API_TOKEN ||
  process.env.CF_API_TOKEN ||
  process.env.CLOUDFLARE_STREAM_TOKEN;

const file = resolve(process.argv[2] || 'docs/assets/vscode-demo.mp4');
const name = process.argv[3] || basename(file);
const apiBase = `https://api.cloudflare.com/client/v4/accounts/${accountId}/stream`;

function fail(message) {
  console.error(message);
  process.exit(1);
}

if (!accountId) {
  fail('Missing CLOUDFLARE_ACCOUNT_ID or CF_ACCOUNT_ID.');
}
if (!token) {
  fail('Missing CLOUDFLARE_API_TOKEN, CF_API_TOKEN, or CLOUDFLARE_STREAM_TOKEN.');
}
if (!existsSync(file)) {
  fail(`Video file not found: ${file}`);
}

const bytes = await readFile(file);
const form = new FormData();
form.append('file', new Blob([bytes], { type: 'video/mp4' }), basename(file));
form.append('meta', JSON.stringify({ name }));

const response = await fetch(apiBase, {
  method: 'POST',
  headers: {
    Authorization: `Bearer ${token}`,
  },
  body: form,
});

const payload = await response.json().catch(() => null);
if (!response.ok || !payload?.success) {
  fail(
    `Cloudflare Stream upload failed: HTTP ${response.status}\n${JSON.stringify(payload, null, 2)}`,
  );
}

const result = payload.result;
const preview = result.preview || null;
const iframe = playerUrlFromPreview(preview, result.uid);
const embedHtml = await fetchEmbedHtml(result.uid);

console.log(
  JSON.stringify(
    {
      uid: result.uid,
      readyToStream: result.readyToStream,
      status: result.status,
      preview,
      stream_iframe_url: iframe,
      embed_html: embedHtml,
      thumbnail: result.thumbnail || null,
      mdx_embed: `<iframe src="${iframe}" title="Reqbook VS Code demo" loading="lazy" allow="accelerometer; gyroscope; autoplay; encrypted-media; picture-in-picture;" allowfullscreen style={{ width: "100%", aspectRatio: "16 / 9", border: 0, borderRadius: "8px" }} />`,
    },
    null,
    2,
  ),
);

function playerUrlFromPreview(preview, uid) {
  if (!preview) {
    return `https://iframe.videodelivery.net/${uid}`;
  }
  try {
    const url = new URL(preview);
    url.pathname = url.pathname.replace(/\/watch\/?$/, '/iframe');
    return url.toString();
  } catch (_) {
    return `https://iframe.videodelivery.net/${uid}`;
  }
}

async function fetchEmbedHtml(uid) {
  const embedResponse = await fetch(`${apiBase}/${uid}/embed`, {
    headers: {
      Authorization: `Bearer ${token}`,
    },
  });
  const text = await embedResponse.text();
  if (!embedResponse.ok) {
    return null;
  }
  try {
    const json = JSON.parse(text);
    return typeof json.result === 'string' ? json.result : text;
  } catch (_) {
    return text;
  }
}
