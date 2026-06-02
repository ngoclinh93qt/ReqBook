import { createRefundQuote } from './refunds/quoteRoute.js';

export const routes = [
  {
    method: 'POST',
    path: '/v1/refunds/quote',
    handler: createRefundQuote,
  },
];
