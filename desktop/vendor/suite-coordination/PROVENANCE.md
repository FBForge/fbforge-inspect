# Coordination source provenance

Source: FBForge Suite commit `6db3fc7ff7ec9fe132ad8285d34757de4936d1ec`,
path `crates/suite-coordination/`. Public source files and tests are byte-for-byte
copies of that commit. This dependency is vendored because the Suite commit is
not yet published. It is not resolved from a moving network branch.

The package manifest is standalone: workspace values are explicit, publication
is disabled, and the minimum Rust version is 1.90. The original source and tests
are unchanged. The application minimum Rust version is not increased.

The operator selected EUPL-1.2 for this newly written FBForge coordination code
as part of this integration. This does not assert that the earlier Suite commit
already declared a license. LICENSE contains the existing public EUPL-1.2 text
from FBForge Connect commit `2afe03c8444e5f48180b214f4afa6aea73ed9be8`.
Only the extra empty final line was removed to match FBForge Inspect's text.
Original license SHA256: `c399f3bf28831d2b28615273b54ca277d5c77125d1e864d6b4d83f57dfc09d96`.
Normalized LICENSE SHA256: `9bfa3666d76c7306f3bc66a70b46d29072e73d5232ac0a144c34ad2bb40947c4`.

## Unchanged source SHA256

- `src/lib.rs`: `136b64b25acd44765b19188fe67de1f29f0519f866e96f2cc8147ded02620231`
- `src/windows.rs`: `1bf02eeb290fdf264284773c5bdaddb338a83f03414340fde7afef780da4e27a`
- `tests/identity.rs`: `263226de112cb3977eb8656abb5ce1db7db27acf69641d3cf37c14b172b1eee5`
- `tests/lease_process.rs`: `3d12a3997a38591c901ce0bdff2774ab6a9fd190ca249a3e6a76e8fd5aea4015`
