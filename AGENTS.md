# Project Rules & Tooling

> [!IMPORTANT]
> **ALWAYS use `bun` and `bunx` instead of `node`, `npm`, and `npx`** for all commands and scripts in this repository.
> • Use `bun <script/file>` instead of `node <script/file>`.
> • Use `bun install`, `bun add`, `bun run` instead of `npm`.
> • Use `bunx <tool>` instead of `npx <tool>` (e.g. `bunx gitnexus analyze`, `bunx --bun shadcn-svelte@latest`).
> • **Do NOT use `node`, `npm`, or `npx` under any circumstances.**

> [!IMPORTANT]
> **ALWAYS read BOTH Long-Term Memory systems (`mem0` AND `agentmemory`) at the start of planning or implementation**:
> • Query `mem0` using `search_memories` / `get_memories`.
> • Query `agentmemory` using `memory_recall` / `memory_smart_search` / `memory_reflect`.
> • You MUST query BOTH memories to retrieve complete session context, past lessons, and design decisions to avoid context loss.
> • Save new learnings and conventions to BOTH memories using `add_memory` (`mem0`) and `memory_save` / `memory_lesson_save` (`agentmemory`).
> • Utilize **GitNexus** tools for codebase exploration and impact analysis as required.

> [!IMPORTANT]
> **TAURI ARCHITECTURE & ABSOLUTE OPTIMIZATION (TAURI SKILL):**
> • Always consult and strictly follow the `/tauri` skill (`/home/arch/.gemini/config/skills/tauri/SKILL.md`) for all Tauri v2 desktop & native app design, implementation, IPC, async runtime, state management, capabilities, and build optimization.
> • **System Architecture & Performance Mindset**: When implementing features, proactively design for maximum system throughput, zero UI jank, non-blocking async loops, minimal memory footprint, and tight capability boundaries.
> • **Maximum Efficiency at QA Gate**: Ensure maximum performance metrics at the QA verification gate — verify non-blocking async handlers (`spawn_blocking` for heavy CPU/blocking I/O), drop mutex locks before `.await`, minimize JSON serialization overhead (use Custom/Asset Protocol or binary/streaming Channel for large payloads), and enforce strict CSP before claiming completion.

> [!IMPORTANT]
> **MANDATORY IMPLEMENTATION HARNESS:** Before any task that creates, changes, fixes, refactors, migrates, or ships behavior, read and follow [`docs/IMPLEMENTATION_COMPLETION_HARNESS.md`](docs/IMPLEMENTATION_COMPLETION_HARNESS.md). Its requirement ledger, adversarial verification, evidence ladder, and Completion Lock control all implementation and remediation work. Do not claim completion while any lock condition remains open.

> [!IMPORTANT]
> **BEST-OF-N IMPLEMENTATION:** For explicitly requested Best-of-N work, or Tier 3 changes with multiple materially different valid implementations, read and follow [`.agents/skills/best-of-n-implementation/SKILL.md`](.agents/skills/best-of-n-implementation/SKILL.md). A ranking winner is only `SELECTED`; final readiness remains controlled by [`docs/IMPLEMENTATION_COMPLETION_HARNESS.md`](docs/IMPLEMENTATION_COMPLETION_HARNESS.md). Missing candidate, ranking, review, or quality-gate evidence produces `INCOMPLETE`, never implicit fallback or approval.

> [!IMPORTANT]
> **AUTOMATED COMMAND EVIDENCE & VERIFICATION HARNESS:**
> • For every coding task in an AGY session, follow [`.agents/rules/command-evidence.md`](.agents/rules/command-evidence.md) and [`docs/agent-harness/README.md`](docs/agent-harness/README.md).
> • Prepare `.agents/task.json` mapping requirements to executable checks before editing.
> • After edits, execute `bun --no-env-file run agent:verify <sessionId>` and require exit code 0 + PASS status on the fresh snapshot before claiming completion.

# Release & Auto-Update Workflow (Cloudflare R2)

> [!IMPORTANT]
> **Always use the All-in-One release script to build and publish updates.**
> • **Script**: `./scripts/release.sh` or `bun run release` (which runs `scripts/publish-release.ts`).
> • **Usage examples**:
>   - Bump version, build, and publish: `./scripts/release.sh --version 0.2.0 --notes "Changelog description"`
>   - Publish current version: `./scripts/release.sh --notes "Hotfix description"`
>   - Re-upload existing build without rebuild: `./scripts/release.sh --skip-build --notes "Updated notes"`
>   - Dry run test: `./scripts/release.sh --dry-run`
> • **Keys & Security**: Signing private key is at `~/.tauri/rustsale.key`. Public key is embedded in `src-tauri/tauri.conf.json`.
> • **Storage & CDN**: Cloudflare R2 bucket `rustsale-releases` (public manifest: `https://rustsale.trantrongtri.com/latest.json`).
> • **Guide**: See [`docs/CLOUDFLARE_R2_RELEASE_GUIDE.md`](docs/CLOUDFLARE_R2_RELEASE_GUIDE.md).

***

<!-- anvien:start -->
# Master iron rules
**Temporary directories created by AI — such as .tmp\ or similar — must be located inside the working repo. Creating temporary directories directly on the C: drive is strictly prohibited.**

**When the user asks to "write/create a plan", the AI agent must immediately use the planner skill and create a real docs/plans plan.**

# AGENTS Rules
0. Anvien is a code-intelligence tool — a broad command system that allows an agent, inside a huge repository, to do almost everything needed for repo work: build graph maps -> identify the right problem -> find the right file/symbol -> verify the right flow -> measure impact scope -> refactor safely -> check API contracts -> audit health, all quickly and accurately.
   - MCP tools are Anvien commands exposed to AI agents.
   - CLI commands are Anvien commands exposed through the terminal.
   - Web/API commands are Anvien runtime commands exposed through the local server.
   - Anvien commands are selected by task. Use the full command set when it gives better evidence.
   - Skills are instruments used with Anvien. Anvien serves the work of other repos/projects.
1. How to use Anvien: run command "anvien --help".
2. As each task is completed, update the corresponding checklist item immediately.
3. Anvien Blast-radius **CRITICAL/HIGH** is only a scope warning that the work must be handled carefully; it is **not** a prohibition against editing code.
4. When SPEC context is required, use `SPEC-MAP.md` as the reading order and read the full relevant SPEC cluster; do not rely on keyword search or partial snippets before concluding, planning, or editing.
5. **Write plan (use planner skill) before coding.**
6. **Code first**; tests should only be updated after the behavior has been correctly implemented in code.
7. Playwright scripts must be reusable under `playwright/`, not one-off temp files; official QA evidence must go to `Reports/qa/playwright/...` as both `.json` and `.md`, while `.tmp` is debug-only.
8. Run a full build before validation.
   - For non-UI changes, validate the changed behavior or contract at its nearest real boundary, record what each command proves, and do not count unrelated, stale, broken, or pass-by-default tests as evidence.
   - For UI behavior changes, open the real user-visible runtime first: the web app in the user's browser or the desktop app on the user's PC. Then record browser or Playwright evidence for the changed behavior.
9. Record benchmark results as each benchmarkable task is completed. Benchmarkable means measured product/runtime performance, capacity, package/startup size, graph/DB throughput, or graph inventory counts; build/test/e2e timings are validation evidence unless the slice changes those systems.
10. Record evidence as each evidenced task is completed.
11. For "doc commits" only, do not use Anvien. When write/edit "doc plan" must use Anvien.
12. After each completed implementation slice, commit the work, then continue until the full plan is complete.
13. Before building, if any process is holding a build-related process or lock, terminate all such processes completely; start the build only after they are gone.
14. (MUST) As soon as the orchestration session begins operation, it must immediately open a visible governance-rule-guard lane using the governance-rule-guard.md skill.

# Anvien - Code Intelligence

Anvien is repo-agnostic: the same command surface works for any indexed repository. Use a repository name/path from `anvien list`, wherever examples refer to `<repo>`.

> If any Anvien command or MCP tool warns that the index is stale, run `anvien analyze --force` from the repository root first.

## Always Do

- **MUST refresh the graph before graph-based work.** Run `anvien analyze --force` before using any Anvien CLI command, MCP tool, MCP resource, Web/API view, or accuracy/benchmark command that reads, queries, validates, mutates, or reports on the semantic graph. This includes `file-detail`, `query`, `context`, `impact`, `detect-changes`, `cypher`, `rename`, `file-hotspots`, MCP `route_map`/`tool_map`/`shape_check`/`api_impact`, CLI `api route-map`/`api tool-map`/`api shape-check`/`api impact`, `augment`, `graph-health`, `query-health`, `resolution-inventory`, `source-site-accuracy`, and `benchmark-compare`.
- **MUST run `file-detail` + impact analysis before editing any function, class, method, exported symbol, API handler, graph builder, resolver, analyzer, or shared contract.**
- **MUST report blast radius.** HIGH or CRITICAL impact means warn clearly and proceed carefully; it is not an automatic ban on editing.
- **MUST run change detection before committing implementation work.** Use MCP `detect_changes` or CLI `anvien detect-changes --repo <repo> --scope all`.
- When exploring unfamiliar code, start with the Anvien command that matches the task instead of defaulting to grep.
- When a command has important flags, check `anvien <command> --help`.

## Never Do

- NEVER edit generated `AGENTS.md` / `CLAUDE.md` content as the permanent source of truth. Update the generator that writes these files, then regenerate.
- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL impact warnings; explain them and keep the change scoped.
- NEVER rename symbols with find-and-replace; use graph-guided rename when available.
- NEVER commit implementation changes without running change detection.
- NEVER reduce graph evidence just to make output smaller; preserve counts, samples, meaning, and traceability.

## Command Selection Guide

Use Anvien by task, not by a fixed workflow. Pick the command surface that matches the job.

| When you need to... | Use |
|---------------------|-----|
| Refresh or rebuild graph evidence | `anvien analyze --force` |
| Analyze a specific local repository path | `anvien analyze <path> --force` |
| Analyze with embeddings enabled | `anvien analyze --embeddings` |
| Record analyze performance/capacity metrics | `anvien analyze --benchmark-json <file>` |
| Check whether the index is current | `anvien status` |
| See indexed repositories and repo names | MCP `list_repos` or CLI `anvien list` |
| Register an existing local index folder | `anvien index [path...]` |
| Delete current repo index | `anvien clean --force` |
| Delete all indexed repo data | `anvien clean --all --force` |
| Find where a concept, behavior, or bug lives | MCP `query` or CLI `anvien query "<concept>" --repo <repo>` |
| Find files relevant to a concept | CLI `anvien query files "<concept>" --repo <repo>` or MCP `query` with `target_type=files` |
| Inspect one symbol deeply | MCP `context` or CLI `anvien context symbol "<symbol>" --repo <repo>` |
| Read detailed information about a file and its relationships with other files. | Prefer CLI `anvien file-detail <path> --repo <repo> --json`; use `anvien context file <path> --repo <repo>` only when you want the context wrapper / human-oriented view |
| Ask structured graph questions | MCP `cypher` or CLI `anvien cypher "<query>" --repo <repo>` |
| Inspect API route handlers, consumers, and linked flows | MCP `route_map` or CLI `anvien api route-map [route] --repo <repo>` |
| Inspect tool handlers and tool-related graph paths | MCP `tool_map` or CLI `anvien api tool-map [tool] --repo <repo>` |
| Check API response shape drift against consumers | MCP `shape_check` or CLI `anvien api shape-check [route] --repo <repo>` |
| Check route/API impact before changing handlers or contracts | MCP `api_impact` or CLI `anvien api impact [route] --repo <repo>` |
| Add graph context to a text search pattern | `anvien augment "<pattern>"` |
| Check symbol blast radius before editing | MCP `impact` or CLI `anvien impact symbol "<symbol>" --repo <repo> --direction upstream` |
| Check file-level blast radius before editing a file | CLI `anvien impact file <path> --repo <repo> --direction upstream` |
| Check changed symbols, files, and affected flows before commit | MCP `detect_changes` or CLI `anvien detect-changes --repo <repo> --scope all`; use `detect-changes files` for grouped file risk |
| Rename a symbol safely | MCP `rename` or CLI `anvien rename <symbol> <newName> --repo <repo>` |
| Audit topology health, diagnostics, and components | `anvien graph-health summary --repo <repo> --json` |
| Explain graph-health for one node | `anvien graph-health explain "File:<path>" --repo <repo> --json` |
| Audit file-level graph health | `anvien graph-health files --repo <repo> --json` or `anvien file-hotspots --repo <repo> --json` |
| Audit query retrieval quality | `anvien query-health --repo <repo>` |
| Audit source-site and resolved-edge accuracy | `anvien source-site-accuracy --graph .anvien/graph.json` |
| Inspect unresolved references / ResolutionGap inventory | `anvien resolution-inventory --graph .anvien/graph.json` |
| Compare analyze benchmark outputs | `anvien benchmark-compare <before> <after>` |
| Start local Web/API runtime | `anvien serve --host 127.0.0.1 --port <port>` |
| Start the MCP server for agents | `anvien mcp` |
| Configure editor/agent integrations | `anvien setup` |
| Inspect local runtime locks and processes | `anvien doctor locks --repo <repo> --json` or `anvien doctor processes --json` |
| Print Anvien version | `anvien version` |
| Generate shell completion scripts | `anvien completion <shell>` |
| Run Claude Code hook integration (hidden lifecycle helper) | `anvien hook claude` |
| Create a repository group | MCP `group_list` for discovery or CLI `anvien group create <name>` |
| Add an indexed repo to a group | `anvien group add <group> <groupPath> <registryName>` |
| Remove repo from a group | `anvien group remove <group> <path>` |
| List or inspect groups | MCP `group_list` or CLI `anvien group list [name]` |
| Check staleness across group repos | MCP `group_status` or CLI `anvien group status <name>` |
| Sync contract registry from indexed group repos | MCP `group_sync` or CLI `anvien group sync <name>` |
| Search execution flows across group repos | MCP `group_query` or CLI `anvien group query <name> "<query>"` |
| Inspect group contract registry | MCP `group_contracts` or CLI `anvien group contracts <name>` |
| Build packaged runtime binary (hidden lifecycle helper) | `anvien package build-runtime` |
| Prepare Go source for package fallback builds (hidden lifecycle helper) | `anvien package prepare-go-source` |
| Verify packaged runtime for this platform (hidden lifecycle helper) | `anvien package ensure-runtime` |
| Remove temporary package source output (hidden lifecycle helper) | `anvien package clean-go-source` |
| Show wiki capability status | `anvien wiki` |
| Show or set wiki capability mode | `anvien wiki-mode [off|local]` |
| Check exact syntax and flags | `anvien <command> --help` |

## Resources

| Resource | Use for |
|----------|---------|
| `anvien://repos` | All indexed repositories |
| `anvien://setup` | MCP setup and tool reference |
| `anvien://repo/<repo>/context` | Codebase overview and index freshness |
| `anvien://repo/<repo>/clusters` | Functional areas / communities |
| `anvien://repo/<repo>/processes` | Execution flows |
| `anvien://repo/<repo>/schema` | Graph schema for Cypher |
| `anvien://repo/<repo>/cluster/{name}` | Functional area details |
| `anvien://repo/<repo>/process/{name}` | Step-by-step execution trace |

## MCP Prompts

| Prompt | Use |
|--------|-----|
| `detect_impact` | Pre-commit impact workflow using `detect_changes`, `context`, and `impact`; HIGH/CRITICAL are blast-radius warnings, not edit bans. |
| `generate_map` | Evidence-backed architecture map workflow; resolves the repo through `anvien://repos` when needed and uses only resources/tools/command output actually read. |

MCP prompts are agent templates, not CLI commands. They guide tool/resource use and must still follow repository rules for freshness, impact-before-edit, and detect-changes before commit.

## Skill Selection Guide

AI agent chooses the skill that fits the work.

| When you need to... | Use |
|---------------------|-----|
| use when user ask to review spec | `.agents/skills/Architect-review/SKILL.md` |
| Data integrity specialist for schema correctness, owner isolation, projections, hash chains, log separation, snapshots, and replay consistency. Use when validating that data remains correct under sync and persistence rules. | `.agents/skills/Data-Integrity/SKILL.md` |
| Edge-case specialist for reconnects, race conditions, duplicate or out-of-order events, stale state, permission bypasses, and fail-closed behavior. Use when trying to break the system under hostile or weird conditions. | `.agents/skills/Edge-Case/SKILL.md` |
| Chuyển spec sản phẩm, tính năng, UI, backend, auth, sync, lifecycle, external contract, hoặc spec đa nhánh thành SVG flow map ngữ nghĩa có metadata máy đọc được, source coverage, render từng flow, gap detection, verification report, và trạng thái BLOCKED hoặc READY_FOR_OWNER_REVIEW. Dùng khi cần biến spec thành flow map, audit độ đầy đủ của spec trước khi code, so sánh với diagram tham chiếu, phơi bày hành vi chưa định nghĩa, hoặc xác định quyết định Owner trước implementation. | `.agents/skills/Spec-to-SVG-Flow-Map/SKILL.md` |
| Software architecture specialist for system design, scalability, and technical decision-making. Use PROACTIVELY when planning new features, refactoring large systems, or making architectural decisions. | `.agents/skills/System-Architect/SKILL.md` |
| Dùng khi user hỏi về UI animation, motion design, gesture, transition, animation review, polish UI, hoặc cải thiện cảm giác chuyển động của app. | `.agents/skills/UI-animation-design/SKILL.md` |
| Create strategic HTML presentations with Chart.js, design tokens, responsive layouts, copywriting formulas, and contextual slide strategies. | `.agents/skills/UI-slides/SKILL.md` |
| Sử dụng khi cần nâng cấp UI UI hiện có, làm UI bớt generic, bớt AI slop, frontend premium, redesign, image-to-code, imagegen web/mobile, brandkit, style variants và full-output. | `.agents/skills/UI-taste-skill/SKILL.md` |
| Use when the user asks to inspect API or MCP surfaces. | `.agents/skills/api-surface/SKILL.md` |
| Use when the user asks to build or change backend code. | `.agents/skills/backend-development/SKILL.md` |
| Design banners for social media, ads, website heroes, creative assets, and print. Multiple art direction options with AI-generated visuals. Actions: design, create, generate banner. Platforms: Facebook, Twitter/X, LinkedIn, YouTube, Instagram, Google Display, website hero, print. Styles: minimalist, gradient, bold typography, photo-based, illustrated, geometric, retro, glassmorphism, 3D, neon, duotone, editorial, collage. Uses ui-ux-pro-max, frontend-design, ai-artist, ai-multimodal skills. | `.agents/skills/banner-design/SKILL.md` |
| Use when the user asks to implement Better Auth. | `.agents/skills/better-auth/SKILL.md` |
| Brand voice, visual identity, messaging frameworks, asset management, brand consistency. Activate for branded content, tone of voice, marketing assets, brand compliance, style guides. | `.agents/skills/brand/SKILL.md` |
| Use when the user asks to integrate Bunny.net. | `.agents/skills/bunny/SKILL.md` |
| Use when the task is to implement code changes from an assigned scope, including bug fixes, follow-ups, rejects, or current-worktree repair. | `.agents/skills/coder/SKILL.md` |
| Use when creating, changing, or reviewing MongoDB/PostgreSQL database work in a real repository, including schema, models, migrations, queries, indexes, transactions, connection config, backup/restore assumptions, or performance; follow repo-native tooling and meet the production database bar unless the user explicitly asks for a prototype. | `.agents/skills/databases/SKILL.md` |
| Use when the user asks to debug. | `.agents/skills/debugging/SKILL.md` |
| Comprehensive design skill: brand identity, design tokens, UI styling, logo generation (55 styles, Gemini AI), corporate identity program (50 deliverables, CIP mockups), HTML presentations (Chart.js), banner design (22 styles, social/ads/web/print), icon design (15 styles, SVG, Gemini 3.1 Pro), social photos (HTML→screenshot, multi-platform). Actions: design logo, create CIP, generate mockups, build slides, design banner, generate icon, create social photos, social media images, brand identity, design system. Platforms: Facebook, Twitter, LinkedIn, YouTube, Instagram, Pinterest, TikTok, Threads, Google Ads. | `.agents/skills/design/SKILL.md` |
| Token architecture, component specifications, and slide generation. Three-layer tokens (primitive→semantic→component), CSS variables, spacing/typography scales, component specs, strategic slide creation. Use for design tokens, systematic design, brand-compliant presentations. | `.agents/skills/design-system/SKILL.md` |
| Use when the user asks to deploy or operate infrastructure. | `.agents/skills/devops/SKILL.md` |
| Use when the user asks to find current technical documentation. | `.agents/skills/docs-seeker/SKILL.md` |
| Use when the user asks to create, edit, or analyze documents. | `.agents/skills/document-skills/docx/SKILL.md` |
| Use when the user asks to design a frontend UI. | `.agents/skills/frontend-design/SKILL.md` |
| Use when the user asks to build or change frontend code. | `.agents/skills/frontend-development/SKILL.md` |
| (MUST RUN) Whenever the orchestration lane is active, this skill must be used to auto open a separate, visible governance session that strictly monitors the Main Orchestration task for rule compliance. | `.agents/skills/governance-rule-guard/SKILL.md` |
| Hướng dẫn chi tiết và chuẩn xác 100% về kiến trúc 3 tầng phân quyền của Google Antigravity, cách cấu hình Global & Project-level Permission Grants kết hợp với Lifecycle Hooks để tự động phê duyệt (Bypass Auto-Approve) vĩnh viễn trên mọi dự án. | `.agents/skills/how-to-allow-approve-antigravity/SKILL.md` |
| Use when the user asks to build an MCP server. | `.agents/skills/mcp-builder/SKILL.md` |
| Use when the user asks to manage MCP integrations. | `.agents/skills/mcp-management/SKILL.md` |
| Use when the user asks to process media files. | `.agents/skills/media-processing/SKILL.md` |
| Use when the user asks to create Mermaid diagrams. | `.agents/skills/mermaidjs-v11/SKILL.md` |
| This skill should be used when the user assigns or asks an agent to become the work orchestrator/main agent for opening and governing separate independent task sessions for subagents. | `.agents/skills/orchestration/SKILL.md` |
| Use for payments, subscriptions, webhooks, refunds, entitlements, or multi-provider orders with SePay, Polar, Stripe, Paddle, or Creem. | `.agents/skills/payment-integration/SKILL.md` |
| Use when the user asks to create, write, or review a docs/plans plan. | `.agents/skills/planner/SKILL.md` |
| Use when creating, reviewing, or troubleshooting Claude Code plugin marketplaces, marketplace.json files, plugin sources, hosting, team allowlists, or install/update failures. | `.agents/skills/plugin-marketplace/SKILL.md` |
| Use when the user asks to solve a hard problem. | `.agents/skills/problem-solving/problem-solving-parent-skill/SKILL.md` |
| Use when the user asks to run QA without fixing code, including mounted runtime behavior, visible user flows, browser-visible app execution, source-of-truth checks, action/state coverage, route/control inventories, Playwright control sweeps, or QA report generation in repositories where Anvien can support | `.agents/skills/qa/SKILL.md` |
| Use when the user asks to refactor code. | `.agents/skills/refactoring/SKILL.md` |
| Use when the user asks to bootstrap a new repo or project. | `.agents/skills/repo-bootstrap/SKILL.md` |
| Use when the user asks to reason step by step. | `.agents/skills/sequential-thinking/SKILL.md` |
| Use when the user asks to build Shopify apps, themes, or extensions. | `.agents/skills/shopify/SKILL.md` |
| Use when the user asks to create or update a skill. | `.agents/skills/skill-creator/SKILL.md` |
| Thảo luận kiến trúc dựa trên SPEC bằng cách đọc tuần tự và đọc hết từng file SPEC trước khi kết luận; trả lại bản nắm bắt kiến trúc gồm invariant, boundary, pipeline, luồng dữ liệu, liên kết thật giữa pipeline, điểm chưa rõ, và câu hỏi thảo luận. Dùng khi user yêu cầu thảo luận kiến trúc, thảo luận về mức độ kiến trúc, bóc tách kiến trúc từ SPEC, hoặc tranh luận một vấn đề kiến trúc dựa trên bộ SPEC. Không dùng để tự cập nhật SPEC, viết code, hoặc review acceptance. | `.agents/skills/spec-architecture-discussion/SKILL.md` |
| Use whenever reviewing completion claims, fixes, diffs, reports, screenshots, or artifacts for acceptance; verify repo/project reality with Anvien evidence. Always use this skill before accepting any agent output, closing any task, or merging any result. | `.agents/skills/supervisor/SKILL.md` |
| Use when the user asks to build 3D web experiences with Three.js. | `.agents/skills/threejs/SKILL.md` |
| Use when binding backend/API data into an already-approved frontend UI without changing approved layout, copy, visual design, component hierarchy, or interaction states. | `.agents/skills/ui-be-binding-skill/SKILL.md` |
| Workflow phát triển phần mềm UI-first cho AI agent. Dùng skill này khi người dùng muốn: xây app mới theo hướng UI trước BE, extract spec từ prototype đã có, chuẩn bị handoff BE implementation từ FE đã code, hoặc nói các cụm như "làm UI trước", "prototype rồi mới spec", "UI-driven", "FE trước BE", "extract contract từ UI", "slot map", "state map", "backend contract map", "chuyển giao kiến trúc sư duyệt". Trigger khi user có HTML prototype hoặc FE components và cần chuyển sang BE. Đây là flow ngược truyền thống — UI là bản đồ thao tác, Spec là luật tối cao, human phải duyệt trước khi implement. | `.agents/skills/ui-driven-spec/SKILL.md` |
| Create beautiful, accessible user interfaces with shadcn/ui components (built on Radix UI + Tailwind), Tailwind CSS utility-first styling, and canvas-based visual designs. Use when building user interfaces, implementing design systems, creating responsive layouts, adding accessible components (dialogs, dropdowns, forms, tables), customizing themes and colors, implementing dark mode, generating visual designs and posters, or establishing consistent styling patterns across applications. | `.agents/skills/ui-styling/SKILL.md` |
| UI/UX design intelligence for web, mobile, and desktop. This skill should be used when designing, building, reviewing, or fixing interfaces, including pages, components, design systems, accessibility, interaction, responsive layout, typography, color, charts, and stack-specific UI implementation. Searchable local data: 79 searchable styles (50 active), 192 product palettes and reasoning profiles, 74 font pairings, 119 UX guidelines, 105 icons, 17 GSAP presets, 25 chart types, and 22 stacks. | `.agents/skills/ui-ux-pro-max/SKILL.md` |
| Use when the user asks to build with Next.js, Turborepo, or web frameworks. | `.agents/skills/web-frameworks/SKILL.md` |
| This skill should be used when a work session begins, before the session selects skills, uses tools, edits files, validates results, or commits work. | `.agents/skills/working-rules/SKILL.md` |

<!-- anvien:end -->

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **RustSale_v2** (9608 symbols, 22869 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## When Debugging

1. `gitnexus_query({query: "<error or symptom>"})` — find execution flows related to the issue
2. `gitnexus_context({name: "<suspect function>"})` — see all callers, callees, and process participation
3. `READ gitnexus://repo/RustSale_v2/process/{processName}` — trace the full execution flow step by step
4. For regressions: `gitnexus_detect_changes({scope: "compare", base_ref: "main"})` — see what your branch changed

## When Refactoring

- **Renaming**: MUST use `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` first. Review the preview — graph edits are safe, text_search edits need manual review. Then run with `dry_run: false`.
- **Extracting/Splitting**: MUST run `gitnexus_context({name: "target"})` to see all incoming/outgoing refs, then `gitnexus_impact({target: "target", direction: "upstream"})` to find all external callers before moving code.
- After any refactor: run `gitnexus_detect_changes({scope: "all"})` to verify only expected files changed.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Tools Quick Reference

| Tool | When to use | Command |
|------|-------------|---------|
| `query` | Find code by concept | `gitnexus_query({query: "auth validation"})` |
| `context` | 360-degree view of one symbol | `gitnexus_context({name: "validateUser"})` |
| `impact` | Blast radius before editing | `gitnexus_impact({target: "X", direction: "upstream"})` |
| `detect_changes` | Pre-commit scope check | `gitnexus_detect_changes({scope: "staged"})` |
| `rename` | Safe multi-file rename | `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` |
| `cypher` | Custom graph queries | `gitnexus_cypher({query: "MATCH ..."})` |

## Impact Risk Levels

| Depth | Meaning | Action |
|-------|---------|--------|
| d=1 | WILL BREAK — direct callers/importers | MUST update these |
| d=2 | LIKELY AFFECTED — indirect deps | Should test |
| d=3 | MAY NEED TESTING — transitive | Test if critical path |

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/RustSale_v2/context` | Codebase overview, check index freshness |
| `gitnexus://repo/RustSale_v2/clusters` | All functional areas |
| `gitnexus://repo/RustSale_v2/processes` | All execution flows |
| `gitnexus://repo/RustSale_v2/process/{name}` | Step-by-step execution trace |

## Self-Check Before Finishing

Before completing any code modification task, verify:
1. `gitnexus_impact` was run for all modified symbols
2. No HIGH/CRITICAL risk warnings were ignored
3. `gitnexus_detect_changes()` confirms changes match expected scope
4. All d=1 (WILL BREAK) dependents were updated

## Keeping the Index Fresh

After committing code changes, the GitNexus index becomes stale. Re-run analyze to update it:

```bash
npx gitnexus analyze
```

If the index previously included embeddings, preserve them by adding `--embeddings`:

```bash
npx gitnexus analyze --embeddings
```

To check whether embeddings exist, inspect `.gitnexus/meta.json` — the `stats.embeddings` field shows the count (0 means no embeddings). **Running analyze without `--embeddings` will delete any previously generated embeddings.**

> Claude Code users: A PostToolUse hook handles this automatically after `git commit` and `git merge`.

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
