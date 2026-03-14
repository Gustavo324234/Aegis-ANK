# Changelog

## [0.6.1] - 2026-03-13
### Fixed
- **core:** reconstruct AST to resolve broken compilation after manual purges ([ANK-912])
- **core:** resolve `anyhow::Context` scoping issues (E0599 and clippy warnings)
- **core:** update `ed25519-dalek` API to v2.0 (`VerifyingKey` compatibility)
- **core:** eradicate `expect_err` using `let Err` syntax (clippy compliance)
- **core:** fix manual string stripping and redundant clones on Copy types
- **core:** box large enum variants in `SwarmError` to reduce memory footprint


## [0.5.1](https://github.com/Gustavo324234/Aegis-ANK/compare/ank-core-v0.5.0...ank-core-v0.5.1) (2026-03-12)


### Features

* **cli:** implement aegis admin cli with gRPC streaming and telemetry ([db4ac5b](https://github.com/Gustavo324234/Aegis-ANK/commit/db4ac5bbf3016e8fa476825744c43cfe8c51fc56))
* **identity:** implement master admin enclave and dynamic tenant ports ([4f89134](https://github.com/Gustavo324234/Aegis-ANK/commit/4f89134ce3d00b6c82d4ab455f35605787c90254))
* **plugins:** implement zero-downtime wasm hot-reloading with atomic rwlocks ([8726ac6](https://github.com/Gustavo324234/Aegis-ANK/commit/8726ac66fde4eae08a847cc35fa453d4f83a60fb))
* **sdk:** create aegis-sdk for wasm tools and refactor standard plugins ([de1aae2](https://github.com/Gustavo324234/Aegis-ANK/commit/de1aae20d21684ef30c023adbd4191393a41ac94))


### Bug Fixes

* **build:** resolve unresolved imports and fix webrtc-vad api usage ([6a3e948](https://github.com/Gustavo324234/Aegis-ANK/commit/6a3e9481b771f6ab856e1d98e39917bc83baff83))
* **core:** plug memory leaks, implement syscall jailing and connection pooling ([1c220e6](https://github.com/Gustavo324234/Aegis-ANK/commit/1c220e6d63ef8c5bab415f6f013ec8f0efa6921b))
* **core:** restore missing core imports and fix type inference ambiguities ([25627ec](https://github.com/Gustavo324234/Aegis-ANK/commit/25627ec3b369bab24ec7fbbaabc2760d942f42d7))
* **core:** update wasmtime preopened_dir api and map missing citadel fields ([99b7d05](https://github.com/Gustavo324234/Aegis-ANK/commit/99b7d05272e0e5ee18c9b2615ad7d74f56573ae7))
* **core:** use pragma_update for SQLCipher key to avoid rusqlite execution error ([cae85a1](https://github.com/Gustavo324234/Aegis-ANK/commit/cae85a1859ddaec4cb75c04b36c1885e1323ab22))
* **security:** mitigate path traversal and insecure hashing ([5308e32](https://github.com/Gustavo324234/Aegis-ANK/commit/5308e32e459c0c4820fffcc34d659ae4419ddef3))
* **server:** resolve E0382 move order for tonic requests ([25f99cd](https://github.com/Gustavo324234/Aegis-ANK/commit/25f99cd700a55a35b6b96d0a76599ac93b0cc1d8))
* **server:** resolve request move conflict by cloning CitadelAuth extensions ([9b9760a](https://github.com/Gustavo324234/Aegis-ANK/commit/9b9760ab19be785961f7ceaccdd46406e5312ed1))
