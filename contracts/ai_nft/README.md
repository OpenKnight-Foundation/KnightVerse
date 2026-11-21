# AI NFT Contract

The `ai_nft` Soroban contract mints AI NFTs, tracks their owners and original
minters, and supports transfers, metadata updates, and burns.

## Current sale and royalty behavior

The contract currently supports `transfer` only; it has no sale or payment
entry point and does not store or pay royalties. A transfer changes the NFT
owner without a payment. The original minter is recorded separately from the
current owner.

## Royalty follow-up (SC-119, #1578)

The royalty work tracked by [SC-119](https://github.com/OpenKnight-Foundation/KnightVerse/issues/1578)
is not implemented yet. Its acceptance criteria are:

- Store a royalty recipient and royalty rate in basis points for each token.
  The minter is the default recipient, and the rate must be capped.
- Add a `sell` or `transfer_with_payment` entry point that transfers the NFT
  and pays the royalty as part of the sale.
- Add tests for the royalty configuration and payment behavior.

The existing `transfer` entry point is a free transfer, not a sale. Do not
assume that it pays a royalty.

## Development

Run the contract tests from the `contracts` directory:

```bash
cargo test -p ai_nft
```
