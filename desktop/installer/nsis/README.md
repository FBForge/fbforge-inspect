# NSIS producer contract

This directory is build tooling, not a runtime downloader. Copy it unchanged into
each cooperating tool's `desktop/installer/nsis` directory. Tauri CLI 2.12.3 and
tauri-utils 2.10.2 support `bundle.useLocalToolsDir` and
`build.beforeBundleCommand`. The tool configurations use both. The before-bundle
command obtains Cargo's actual target directory with locked, offline metadata.

`node desktop/installer/nsis/toolchain.mjs prepare` prepares
`<cargo-target>/.tauri/NSIS`. It checks the public NSIS 3.13 ZIP and Tauri plugin by
SHA256, extracts the fixed archive, replaces the six Unicode stubs, checks all
compiler/stub bytes and a closed digest of all 442 distribution files, and publishes
that private build toolset. Includes, plugins and additional files participate in
the digest. A prior toolset is
preserved under a separate name. It never edits the user's global Tauri cache.
The compiler-time producer checks the toolset again. Wrong or missing bytes stop
compilation. No signing key is read by these scripts.

The source patch changes one pre-extraction decision in NSIS 3.13: an unavailable,
truncated or unusable `GetTempPathW` result stops setup. Setup cannot switch to the
Windows or Low temporary directories. Native Suite launch separately checks and
holds the supported directory's identity and ACL through observed child exit.
This patch does not provide that native authorization by itself. Direct manual
setup remains restricted to a suitably protected user TEMP directory. End-user
Windows execution and positive signed installation are separate acceptance gates.

The six stubs were built from the public upstream commit recorded in
`manifest.json`, with the included exact patch and source date. They are PE stub
data for makensis, not standalone programs. `COPYING` contains upstream licensing.
`build-inputs.json` records the source, SCons and MinGW package inputs. To rebuild
inside an offline scratch-only sandbox with those input files:

```sh
sh installer/nsis/rebuild.sh /inputs /scratch/rebuilt-stubs
```

The script uses existing Python 3, dpkg-deb and host GNU build tools, installs
nothing globally, and refuses output that differs from the reviewed six hashes.
The original bounded build used Python 3.14.4, host GNU 15.2.0, MinGW GCC 13-posix
and SCons 4.8.1. The source compilation passed. The convenience rebuild wrapper is
not a claim of independent second-build reproducibility.

During makensis, `create-payload.mjs INPUT OUTPUT DIGEST_DEFINITION NSIS_DIRECTORY`
receives the actual rendered `File` map after application signing. Input is a
UTF-8 response file with one JSON header and alternating absolute source/relative
destination lines. Paths remain data. Output is the bounded native schema1
payload manifest. A separate file contains exactly one generated NSIS definition
with its lowercase SHA256. That value becomes the signed VERSIONINFO key
`FBForgePayloadManifestSha256`. Setup embeds the exact manifest bytes as
`$PLUGINSDIR\fbforge-payload.v1.json`. The native adapter independently checks the
signed host, expected key, digest, identity, version and selected components.

The manifest hash binds bytes to the signed host. It grants no target, execution,
process-absence or mutation permission. Native lease, private target preparation,
stage health, publication and finalization are required separately.

`npm run test:installer` covers the byte inventory, path/component rejection,
actual toolset preparation, changed includes/stubs, and the compiler CLI digest.
Tests download only the SHA256-pinned public build inputs by default. An offline
runner can set `FBFORGE_NSIS_FIXTURE_DIRECTORY` to its read-only directory containing
`nsis-3.13.zip` and `nsis_tauri_utils.dll` instead.

Primary sources:

- [NSIS 3.13 source](https://github.com/NSIS-Dev/nsis/tree/a4ba28e60bd9e7c271cd2bfc1558889ce3355653)
- [Tauri CLI 2.12.3 NSIS bundler](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.3/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs)
- [Tauri 2.10.2 build configuration](https://docs.rs/tauri-utils/2.10.2/tauri_utils/config/struct.BuildConfig.html)
