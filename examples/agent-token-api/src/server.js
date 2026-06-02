import { createServer } from 'node:http';
import { routes } from './routes.js';
import { sendJson } from './shared/http.js';

const port = Number(process.env.PORT || 7878);

const server = createServer(async (req, res) => {
  const url = new URL(req.url || '/', `http://${req.headers.host || 'localhost'}`);
  const route = routes.find((candidate) => {
    return candidate.method === req.method && candidate.path === url.pathname;
  });

  if (!route) {
    const samePath = routes.some((candidate) => candidate.path === url.pathname);
    sendJson(res, samePath ? 405 : 404, {
      error: samePath ? 'method_not_allowed' : 'not_found',
      message: samePath ? 'Use the documented HTTP method for this endpoint.' : 'No route matches this path.',
    });
    return;
  }

  try {
    await route.handler(req, res);
  } catch (error) {
    const status = error.statusCode || 500;
    sendJson(res, status, {
      error: error.code || 'internal_error',
      message: error.expose ? error.message : 'Unexpected server error.',
      ...(error.fields ? { fields: error.fields } : {}),
    });
  }
});

server.listen(port, '127.0.0.1', () => {
  console.log(`Agent token benchmark API listening on http://127.0.0.1:${port}`);
});
