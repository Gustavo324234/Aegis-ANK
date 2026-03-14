# Changelog

## [0.2.2] - 2026-03-13

### Bug Fixes
* **server:** Resolve `webrtc-vad` thread-safety via `Send` wrapper and `Arc<Mutex>`.
* **server:** Fix gRPC `Streaming` test initialization with `MockDecoder` and `http-body-util`.
* **server:** Resolve `ank-server` test compilation (missing fields in `TaskRequest`).
* **core:** Implement `Default` for `MasterEnclave` and fix re-exports for testing.
* **core:** Fix Clippy warnings (duplicated attributes in `native.rs` and items order in `master.rs`).
* **build:** Add missing workspace dependencies `bytes`, `http-body-util` and `async-stream`.


## [0.2.1](https://github.com/Gustavo324234/Aegis-ANK/compare/ank-server-v0.2.0...ank-server-v0.2.1) (2026-03-12)


### Features

* **cli:** implement aegis admin cli with gRPC streaming and telemetry ([db4ac5b](https://github.com/Gustavo324234/Aegis-ANK/commit/db4ac5bbf3016e8fa476825744c43cfe8c51fc56))
* **identity:** implement master admin enclave and dynamic tenant ports ([4f89134](https://github.com/Gustavo324234/Aegis-ANK/commit/4f89134ce3d00b6c82d4ab455f35605787c90254))
* **plugins:** implement zero-downtime wasm hot-reloading with atomic rwlocks ([8726ac6](https://github.com/Gustavo324234/Aegis-ANK/commit/8726ac66fde4eae08a847cc35fa453d4f83a60fb))
* **sdk:** create aegis-sdk for wasm tools and refactor standard plugins ([de1aae2](https://github.com/Gustavo324234/Aegis-ANK/commit/de1aae20d21684ef30c023adbd4191393a41ac94))
* **siren:** implement asynchronous zero-blocking zero-panic TTS worker and SentenceAccumulator ([99e50f4](https://github.com/Gustavo324234/Aegis-ANK/commit/99e50f43a0ca54e1e630b2e3cea8be3c231e7b37))


### Bug Fixes

* **build:** resolve unresolved imports and fix webrtc-vad api usage ([6a3e948](https://github.com/Gustavo324234/Aegis-ANK/commit/6a3e9481b771f6ab856e1d98e39917bc83baff83))
* **core:** plug memory leaks, implement syscall jailing and connection pooling ([1c220e6](https://github.com/Gustavo324234/Aegis-ANK/commit/1c220e6d63ef8c5bab415f6f013ec8f0efa6921b))
* **security:** mitigate path traversal and insecure hashing ([5308e32](https://github.com/Gustavo324234/Aegis-ANK/commit/5308e32e459c0c4820fffcc34d659ae4419ddef3))
* **server:** resolve E0382 move order for tonic requests ([25f99cd](https://github.com/Gustavo324234/Aegis-ANK/commit/25f99cd700a55a35b6b96d0a76599ac93b0cc1d8))
* **server:** resolve request move conflict by cloning CitadelAuth extensions ([9b9760a](https://github.com/Gustavo324234/Aegis-ANK/commit/9b9760ab19be785961f7ceaccdd46406e5312ed1))
