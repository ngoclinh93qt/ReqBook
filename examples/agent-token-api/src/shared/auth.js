import { ApiError } from './errors.js';

const expectedToken = process.env.SUPPORT_TOKEN || 'bench_support_token';

export function requireSupportBearer(headers) {
  const authorization = headers.authorization || '';
  const [scheme, token] = authorization.split(/\s+/, 2);

  if (scheme !== 'Bearer' || !token) {
    throw new ApiError(401, 'missing_bearer_token', 'Authorization header must use Bearer auth.');
  }

  if (token !== expectedToken) {
    throw new ApiError(401, 'invalid_token', 'Bearer token is not authorized for refund quotes.');
  }
}
