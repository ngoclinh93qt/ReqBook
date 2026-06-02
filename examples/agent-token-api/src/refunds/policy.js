export const refundReasons = new Set([
  'duplicate',
  'damaged',
  'customer_request',
  'late_delivery',
]);

export const maxLineItems = 25;
export const maxQuantityPerLine = 10;
export const maxShippingRefundCents = 2500;
export const quoteTtlSeconds = 900;
export const approvalThresholdCents = 50000;
export const approvalQuantityThreshold = 5;
export const restockingFeeRate = 0.15;
export const restockingFeeCapCents = 2500;

export function shouldWaiveRestockingFee(reason) {
  return reason === 'damaged' || reason === 'late_delivery';
}
