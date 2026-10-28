# Third-party notices

FBForge Inspect is licensed under EUPL-1.2. The included components retain their own licenses.

| Component | Version | License | Project |
| --- | --- | --- | --- |
| Vue | 3.5.43 | MIT | https://github.com/vuejs/core |
| Tauri / Tauri API | 2.12.4 | MIT OR Apache-2.0 | https://github.com/tauri-apps/tauri |
| Tauri dialog plugin | 2.8.3 | MIT OR Apache-2.0 | https://github.com/tauri-apps/plugins-workspace |
| Tokio | 1.53.3 | MIT | https://github.com/tokio-rs/tokio |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| Gson | 2.13.2 | Apache-2.0 | https://github.com/google/gson |
| Nimbus JOSE + JWT | 10.10 | Apache-2.0 | https://connect2id.com/products/nimbus-jose-jwt |
| Bouncy Castle bcprov / bcpkix / bcutil | 1.86 | Bouncy Castle license (MIT style) | https://www.bouncycastle.org/licence.html |
| OpenJDK runtime | 25 | GPL-2.0 with Classpath Exception | https://openjdk.org/legal/ |

The runtime's `legal/` directory contains its module license texts. Java libraries retain license resources inside the engine JAR. Transitive Rust and frontend dependencies are pinned by Cargo.lock and package-lock.json. Generated distribution notices and license texts are collected by `scripts/collect-licenses.mjs` before packaging.

The prepared Windows x64 build uses Eclipse Temurin OpenJDK 25.0.5+8. Its corresponding source archive is available from the [official Temurin release](https://github.com/adoptium/temurin25-binaries/releases/download/jdk-25.0.5%2B8/OpenJDK25U-jdk-sources_25.0.5_8.tar.gz). The runtime's `release` file records the bundled version. Rebuilders must retain the notices and supply the corresponding source for their chosen runtime distribution when distributing binaries.

`licenses/components.json` describes the most recently collected target. The target-specific `components.<target>.json` files preserve the Linux development and Windows distribution inventories. Component directories may include notices for both platforms.

The logo was created for FBForge Inspect using image generation. It is included under the project license. FIT-Connect and PVOG names identify interoperable third-party services. Their operators do not endorse this application.

Public protocol specifications are linked in docs/SOURCES.md. Their implementation is independent. Documentation text in this repository is original.
