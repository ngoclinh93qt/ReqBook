# Ecommerce checkout flow example

This example is a business workflow written as executable Markdown. It works
well with `rqb mock` because every endpoint includes a recorded expected
response.

```bash
rqb validate api-docs/
rqb mock api-docs --port 4001
rqb flow api-docs/flows/cart-to-checkout.md
rqb serve --mock
```

Agent prompt:

```text
Use examples/ecommerce-checkout-flow/api-docs to add a discount-code endpoint.
Update the cart-to-checkout flow so the discount is applied before checkout, and
keep assertions stable enough for CI.
```
