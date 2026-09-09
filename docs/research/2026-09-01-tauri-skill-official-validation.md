# Tauri skill — official-source validation

Date: 2026-09-01  
Target reviewed: `/home/arch/.agents/skills/tauri/SKILL.md`  
Scope: Tauri v2, Cargo/rustc, Tokio, WebView2, and Tauri distribution guidance. Only primary/official sources were used.

## Executive verdict

The skill should be reorganized as a rewrite, not lightly reordered. Its useful advice is mixed with duplicated sections, stale Tauri v1/v2-beta configuration, overbroad security and performance claims, non-compiling examples, and an uncited SEO-style appendix. The strongest parts are least-privilege capabilities, measuring before optimizing, bounded IPC, short lock lifetimes, and release-profile tuning—but each needs tighter conditions.

Remove unsupported percentage claims and anecdotal recommendations. Make optimization goal-driven: security, binary size, startup, throughput, and build speed frequently require different choices.

## Exact and near-duplicate material

| Repeated idea | Locations in current skill | Consolidation |
|---|---|---|
| Release profile (`codegen-units`, LTO, `opt-level`, `panic`, `strip`) | §3, “Tauri Performance Optimization Guide” §4, and “Basic Strategies” | Keep one Cargo profiles section with separate size-first, balanced, and speed-first choices. |
| Offload blocking/CPU work with `spawn_blocking` | §5 and copied “Rust Backend” section | Keep one async/runtime section; add bounded CPU concurrency. |
| Capabilities and least privilege | §§1, 7, 8, 9, 11, security checklist, core principle | Merge into one security boundary chapter: registration → capability → permission → scope → origin/window targeting. |
| Large IPC data | §4 pagination and copied “Efficient IPC” section | Replace with one transport-selection decision tree: JSON invoke, raw response, channel, event, or asset protocol. |
| Memory/streaming advice | §4, copied performance guide, SEO appendix | Keep one evidence-based section; remove generic percentages. |
| Platform-specific WebView behavior | §6, general optimization strategy, SEO appendix | Keep a conditional platform matrix. |
| Measurement/profiling | §14, “Measure Before Optimize,” rendering advice | Put a measurement gate before all optimizations. |
| Frontend bundle/render advice | Performance guide and SEO appendix | Keep only framework-neutral, measurable guidance; link framework docs when used. |

## Verified conflict matrix

| Topic | Finding | Required correction / safe guidance | Official sources |
|---|---|---|---|
| Capabilities are a “security sandbox” | Overstated. Capabilities are an ACL boundary; overlapping capabilities merge permissions. They do not protect insecure Rust code, loose scopes, or incorrect authorization checks. | Call this an authorization boundary. Audit all overlapping capabilities and command-side scope enforcement. | [Capabilities](https://v2.tauri.app/security/capabilities/), [Runtime Authority](https://v2.tauri.app/security/runtime-authority/), [Scopes](https://v2.tauri.app/security/scope/) |
| “Capability files must be referenced in `app.security.capabilities`” | Incorrect. Files in `src-tauri/capabilities` are auto-enabled by default; the config list is used to explicitly select a subset. | Recommend an explicit list for auditable production configuration, but do not label it mandatory. | [Capabilities configuration](https://v2.tauri.app/security/capabilities/), [SecurityConfig](https://v2.tauri.app/reference/config/#securityconfig) |
| “Every command must be declared in a capability” | Incorrect for bundled local origins. Registered custom commands are accessible to app windows/webviews by default unless the app registers them in `AppManifest::commands` for ACL enforcement. Plugin/core commands are capability-controlled. | Distinguish command registration (`generate_handler!`) from ACL registration (`AppManifest::commands`) and capability grants. Remote origins are ACL-checked in current Tauri; require Tauri 2.11.1+ because this boundary had a security fix. | [Application commands in capabilities](https://v2.tauri.app/security/capabilities/#application-commands), [Permissions](https://v2.tauri.app/security/permissions/), [Tauri 2.11.1 security release](https://v2.tauri.app/release/tauri/v2.11.1/) |
| Platform target `"macos"` | Stale/invalid casing. Current enum is `macOS`. | Use only documented values: `linux`, `macOS`, `windows`, `iOS`, `android`. | [Target enum](https://v2.tauri.app/reference/config/#target) |
| Combined command `bun tauri add fs shell dialog notification` | Invalid CLI usage: `tauri add` accepts one plugin argument. | Run one command per plugin. For opening URLs/files, prefer the newer opener plugin where applicable. | [Tauri CLI `add`](https://v2.tauri.app/reference/cli/), [Shell plugin](https://v2.tauri.app/plugin/shell/), [Opener plugin](https://v2.tauri.app/plugin/opener/) |
| “Strict CSP” containing `script-src 'unsafe-inline'` | Self-contradictory and weakens script protection. | Start restrictive; add only directives proven necessary. Use `devCsp` for development exceptions. Avoid disabling Tauri’s CSP asset modification except with an explicit threat review. | [CSP](https://v2.tauri.app/security/csp/), [SecurityConfig](https://v2.tauri.app/reference/config/#securityconfig) |
| Isolation pattern | Directionally correct, but config uses stale `devPath`; isolation has overhead and Windows sandboxed-iframes cannot load ES modules. | Use `build.devUrl` and `build.frontendDist`. Recommend isolation for suitable threat models, with a minimal isolation app and compatibility/performance verification. Isolation does not replace capabilities or CSP. | [Isolation pattern](https://v2.tauri.app/concept/inter-process-communication/isolation/), [BuildConfig](https://v2.tauri.app/reference/config/#buildconfig) |
| Remote URL capability described as allowing navigation | Conceptually wrong. A remote capability grants Tauri API access to matching remote origins; navigation is configured separately. Linux/Android cannot reliably distinguish iframe requests from the window. | Deny remote API access by default. If required, pin exact origins, grant minimal permissions, and test per platform. Keep development origins out of production capabilities. | [Remote API access](https://v2.tauri.app/security/capabilities/#remote-api-access), [SecurityConfig](https://v2.tauri.app/reference/config/#securityconfig) |
| `devPath` in a Tauri v2 example | Stale v1/early migration key. | Replace with `build.devUrl`; bundled files use `build.frontendDist`. | [Tauri v2 config](https://v2.tauri.app/reference/config/#buildconfig), [Vite integration](https://v2.tauri.app/start/frontend/vite/), [v2 beta rename](https://v2.tauri.app/release/tauri/v2.0.0-beta.0/) |
| `removeUnusedCommands` | Valid but incomplete guidance. It removes registered plugin commands never allowed by the static capability ACL and does not account for dynamic ACLs. | Enable only when capabilities are static and supported plugin/build versions are in use; do not describe it as general dead-code removal for app commands. | [Reducing app size](https://v2.tauri.app/concept/size/), [BuildConfig](https://v2.tauri.app/reference/config/#buildconfig) |
| “IPC heavily serializes JSON” / “never send large files through IPC” | Too absolute. Normal serializable invoke values use JSON, but Tauri supports raw request/response bodies; channels are optimized for ordered streaming. Events are JSON-string based and intended for small/broadcast data. | Use structured invoke for ordinary values, `tauri::ipc::Response` for a large binary response, `Channel` for ongoing ordered chunks/progress, events for small notifications, and asset protocol for webview-consumed files/media. | [Calling Rust](https://v2.tauri.app/develop/calling-rust/), [Calling the frontend / channels](https://v2.tauri.app/develop/calling-frontend/), [`Request`](https://docs.rs/tauri/latest/tauri/ipc/struct.Request.html), [`Response`](https://docs.rs/tauri/latest/tauri/ipc/struct.Response.html) |
| `convertFileSrc` as a direct optimization | Incomplete security model. Conversion does not authorize access. | Require `app.security.assetProtocol.enable`, a narrow asset scope, and the correct CSP source (`asset:` or `http://asset.localhost`). Persist only user-approved dynamic paths when necessary. | [`convertFileSrc`](https://v2.tauri.app/reference/javascript/api/namespacecore/#convertfilesrc), [Asset protocol](https://v2.tauri.app/security/asset-protocol/) |
| Async commands using `std::fs`, direct CPU work, or long synchronous operations | Contradicts the skill’s own non-blocking rule. Async syntax alone does not make blocking code non-blocking. | Use async I/O where practical or `tauri::async_runtime::spawn_blocking`. Bound parallel CPU work with a semaphore or a dedicated CPU pool; Tokio’s blocking pool has a large upper limit and blocking tasks generally cannot be aborted after starting. | [Calling Rust: async commands](https://v2.tauri.app/develop/calling-rust/), [Tauri `spawn_blocking`](https://docs.rs/tauri/latest/tauri/async_runtime/fn.spawn_blocking.html), [Tokio `spawn_blocking`](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html) |
| Async state must use an async mutex | Not universally true. Tokio explicitly notes that `std::sync::Mutex` is often preferred for data-only critical sections when the guard never crosses `.await`; async mutexes are for guards that must cross `.await`, commonly I/O resources. | State the lock invariant: short critical section, never hold a blocking guard across `.await`, minimize contention, and avoid redundant `Arc` because Tauri state is already shared. | [Tauri state management](https://v2.tauri.app/develop/state-management/), [Tokio `Mutex`](https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html) |
| One release profile is “smaller and faster” | Incorrect trade-off framing. `opt-level="z"`, `"s"`, and `3` optimize different goals; `z` disables loop vectorization, and `s`/`z` are not guaranteed smallest. `lto=true` is equivalent to fat LTO; ThinLTO trades some optimization for link time. | Provide selectable profiles and require measurement. Mention that workspace profiles are read only from the workspace root. Explain semantic costs: `panic="abort"` disables unwinding; stripping reduces post-crash symbolization. | [Tauri app size](https://v2.tauri.app/concept/size/), [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html) |
| Brotli asset compression can be turned on/off casually for startup | Unsupported as written. Tauri’s default crate features include compression and embedded assets are compressed at compile time; installer compression is a separate concern. There is no simple `tauri.conf.json` Brotli toggle. | Do not prescribe disabling compression without a feature-graph audit and benchmark. Separate embedded-asset compression from NSIS/RPM installer compression. | [Tauri crate features](https://docs.rs/crate/tauri/latest/source/Cargo.toml.orig), [EmbeddedAssets](https://docs.rs/tauri/latest/tauri/webview/struct.EmbeddedAssets.html), [NSIS compression config](https://v2.tauri.app/reference/config/#nsisconfig) |
| `RUSTFLAGS="-C link-arg=-fuse-ld=lld"` for both Windows and Linux | Unsafe universal advice. `-fuse-ld=lld` is a compiler-driver convention, target linkers differ, and global `RUSTFLAGS` may affect host build scripts/proc macros. Current Rust targets also have evolving default linker behavior. | Use target-specific Cargo config, validate the platform toolchain, and benchmark CI/release artifacts. Never present one linker flag as portable across GNU Linux and MSVC Windows. | [Cargo target linker/rustflags](https://doc.rust-lang.org/cargo/reference/config.html#targettriplelinker), [rustc linker options](https://doc.rust-lang.org/rustc/codegen-options/index.html#linker), [Linker-plugin LTO requirements](https://doc.rust-lang.org/stable/rustc/linker-plugin-lto.html) |
| `WEBKIT_DISABLE_DMABUF_RENDERER=1` as a general Linux optimization | Wrong default. It is a workaround for affected Linux graphics/driver failures and disables a faster rendering path. | Apply only to confirmed affected systems and document the performance cost. | [Tauri Linux graphics debugging](https://v2.tauri.app/develop/debug/linux-graphics/) |
| `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-features=msWebOOUI` in production | Unsupported and unsafe as baseline guidance. Microsoft documents additional browser flags for testing/diagnostics and says not to use them in production. | Remove from the standard skill; allow only a named, verified workaround with rollback and release testing. | [WebView2 feature flags](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags) |
| UPX as a normal Tauri size optimization | No official Tauri recommendation supports it; the copied percentage/anecdote is unverifiable. Binary packing can interfere with signing, updater assumptions, antivirus reputation, and crash diagnosis. | Remove from core guidance. If retained, label experimental: pack before signing, then sign/verify the final artifact and test updater, installer, AV, and runtime on every target. | [Tauri distribution signing overview](https://v2.tauri.app/distribute/), [Windows code signing](https://v2.tauri.app/distribute/sign/windows/), [macOS signing](https://v2.tauri.app/distribute/sign/macos/), [UPX project](https://github.com/upx/upx) |
| “Static Linux builds avoid distro dependency issues” | Misleading for a normal Tauri GUI. Tauri relies on WebKitGTK/GTK; AppImage bundles many dependencies but still has glibc compatibility constraints and should be built on the oldest supported base system. | Recommend a tested package matrix (native packages/AppImage/Flatpak/Snap as appropriate). Do not promise fully static or musl portability for ordinary Tauri GUI apps. | [Tauri AppImage distribution](https://v2.tauri.app/distribute/appimage/), [Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux) |

## Internal example defects to fix during rewrite

- The pagination example consumes `entries` with `into_iter()` and then reads `entries.len()`, which is a Rust move error. It also performs blocking `std::fs::read_dir` inside an async command.
- `read_config` uses blocking `std::fs::read_to_string` and path checks inside an async command.
- `expensive_operation` calls CPU-heavy work directly from async code despite the adjacent `spawn_blocking` rule.
- `get_config` returns `*guard`, which only works if the stored value is `Copy`; use an owned clone/DTO or another explicit ownership strategy.
- `Database::new()?` appears in a `fn main()` example that does not return `Result`, so the snippet is not self-consistent.
- The filesystem capability identifier says “recursive,” while the shown `$APPDATA/*` glob denotes only the displayed path pattern; use explicit, minimal scopes and test the intended depth.
- `[profile.dev] debug = true` merely restates Cargo’s default and should not be presented as an optimization.

## Genuine owner-choice conflicts

These are policy decisions, not facts that ordering alone can resolve:

1. **Default release objective:** size-first (`s`/`z`), throughput-first (`2`/`3`), or a balanced template with benchmark gates. Recommended: balanced template plus measured alternatives; do not promise one profile wins both size and speed.
2. **Remote content policy:** forbid remote API origins by default, or include a guarded remote-origin recipe. Recommended: forbid by default and place the recipe in an advanced security section.
3. **Isolation policy:** default-on where compatible, or optional high-security hardening. Recommended: strongly recommend for higher-risk IPC, with explicit Windows module and performance compatibility checks.
4. **UPX:** remove entirely, or retain as an experimental appendix. Recommended: remove; it is not official Tauri optimization guidance.
5. **Linux distribution promise:** tested package/AppImage matrix, or an attempted static-build workflow. Recommended: package matrix; remove the “static avoids dependency issues” promise.

## Recommended final skill structure

1. Trigger and supported Tauri version
2. Measurement gate and optimization objective
3. Security boundary: command registration, capabilities, permissions, scopes, origins, windows
4. CSP, isolation, remote content, and asset protocol
5. IPC transport decision tree
6. Async work, CPU pools, state, and lock invariants
7. Frontend/webview performance with platform-conditional workarounds
8. Cargo release profiles, feature audit, bundle and installer size
9. Platform packaging, signing, updater, and Linux compatibility
10. Verification checklist and official-source index

The copied SEO article should not remain as a second guide inside the skill. Any claim without an official source or project benchmark should be removed or labeled as a hypothesis to measure.
