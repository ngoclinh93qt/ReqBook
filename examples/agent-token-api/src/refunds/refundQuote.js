import { ApiError } from '../shared/errors.js';
import {
  approvalQuantityThreshold,
  approvalThresholdCents,
  maxLineItems,
  maxQuantityPerLine,
  maxShippingRefundCents,
  quoteTtlSeconds,
  refundReasons,
  restockingFeeCapCents,
  restockingFeeRate,
  shouldWaiveRestockingFee,
} from './policy.js';

function validateQuoteRequest(body) {
  const fields = {};

  if (typeof body.orderId !== 'string' || !/^ord_[0-9]+$/.test(body.orderId)) {
    fields.orderId = 'Must match ord_<digits>.';
  }

  if (!Array.isArray(body.lineItems) || body.lineItems.length === 0) {
    fields.lineItems = 'Must include at least one line item.';
  } else if (body.lineItems.length > maxLineItems) {
    fields.lineItems = `Cannot include more than ${maxLineItems} line items.`;
  } else {
    body.lineItems.forEach((item, index) => {
      const prefix = `lineItems.${index}`;
      if (!item || typeof item !== 'object') {
        fields[prefix] = 'Line item must be an object.';
        return;
      }
      if (typeof item.sku !== 'string' || item.sku.trim() === '') {
        fields[`${prefix}.sku`] = 'SKU is required.';
      }
      if (!Number.isInteger(item.quantity) || item.quantity < 1 || item.quantity > maxQuantityPerLine) {
        fields[`${prefix}.quantity`] = `Quantity must be an integer from 1 to ${maxQuantityPerLine}.`;
      }
      if (!Number.isInteger(item.unitPriceCents) || item.unitPriceCents < 0) {
        fields[`${prefix}.unitPriceCents`] = 'Unit price must be a non-negative integer.';
      }
    });
  }

  if (typeof body.reason !== 'string' || !refundReasons.has(body.reason)) {
    fields.reason = 'Must be duplicate, damaged, customer_request, or late_delivery.';
  }

  if (
    body.shippingRefundCents !== undefined &&
    (!Number.isInteger(body.shippingRefundCents) ||
      body.shippingRefundCents < 0 ||
      body.shippingRefundCents > maxShippingRefundCents)
  ) {
    fields.shippingRefundCents = `Must be an integer from 0 to ${maxShippingRefundCents}.`;
  }

  if (Object.keys(fields).length > 0) {
    throw new ApiError(422, 'validation_error', 'Refund quote request failed validation.', { fields });
  }
}

export function calculateRefundQuote(body) {
  validateQuoteRequest(body);

  const subtotalRefundCents = body.lineItems.reduce((sum, item) => {
    return sum + item.quantity * item.unitPriceCents;
  }, 0);
  const shippingRefundCents = body.shippingRefundCents || 0;
  const totalQuantity = body.lineItems.reduce((sum, item) => sum + item.quantity, 0);
  const restockingFeeCents = shouldWaiveRestockingFee(body.reason)
    ? 0
    : Math.min(Math.round(subtotalRefundCents * restockingFeeRate), restockingFeeCapCents);
  const totalRefundCents = subtotalRefundCents + shippingRefundCents - restockingFeeCents;

  if (totalRefundCents <= 0) {
    throw new ApiError(422, 'policy_rejected', 'Refund quote total must be greater than zero.');
  }

  return {
    quoteId: `rfq_${body.orderId}_${body.reason}`,
    orderId: body.orderId,
    currency: 'USD',
    subtotalRefundCents,
    shippingRefundCents,
    restockingFeeCents,
    totalRefundCents,
    expiresInSeconds: quoteTtlSeconds,
    requiresApproval:
      totalRefundCents > approvalThresholdCents || totalQuantity > approvalQuantityThreshold,
  };
}
