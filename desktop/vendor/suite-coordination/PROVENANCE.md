# Coordination source provenance

Source: FBForge Suite commit `f4b6cf322feeca31e4a908482486d03ef7d70138`, path `crates/suite-coordination/`.
All public Rust sources and tests below are byte-for-byte copies of this fixed
local source snapshot. No moving network branch is resolved.

The standalone manifest keeps explicit workspace values and the existing package
version/minimum Rust version. The copied implementation adds the fixed pending
installation check and native lease identity accessor used by this integration.
Kernel identity, legacy application ID and coordination protocol remain stable.

The existing EUPL-1.2 LICENSE is retained without changes.

## Source SHA256

- `src/installation.rs`: `259c3f83346cd853d40309a85af703c39956712080eafa4b3664e46756126cca`
- `src/lib.rs`: `284e131410b4032026e54a40e4f67a668bd457abf321a3f935ef424595c09c42`
- `src/windows.rs`: `63a5534eadae6df7049fe61b7ee2f7fc223ea7bf0233dd04cf80a3381b06c891`
- `tests/identity.rs`: `263226de112cb3977eb8656abb5ce1db7db27acf69641d3cf37c14b172b1eee5`
- `tests/interrupted_installation.rs`: `8afc4a870950ecd3b7b48c636cd7ec7aa0b0b3a0dadcfb072a8e54c2ed167f41`
- `tests/lease_process.rs`: `3d12a3997a38591c901ce0bdff2774ab6a9fd190ca249a3e6a76e8fd5aea4015`
