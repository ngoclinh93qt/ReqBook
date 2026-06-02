import { requireSupportBearer } from '../shared/auth.js';
import { readJson, sendJson } from '../shared/http.js';
import { calculateRefundQuote } from './refundQuote.js';

export async function createRefundQuote(req, res) {
  requireSupportBearer(req.headers);
  const body = await readJson(req);
  const quote = calculateRefundQuote(body);
  sendJson(res, 201, quote);
}
