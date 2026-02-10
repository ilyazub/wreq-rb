# Migrate rquest-rb to wreq-rb: Full Upstream Migration + http.rb API Parity

## TL;DR

> **Quick Summary**: Migrate the `rquest-rb` Ruby gem from the deprecated `rquest` Rust crate to its successor `wreq`, fully rename the gem to `wreq-rb`, refactor the Rust native extension for safety and maintainability, and achieve full API parity with the `http.rb` gem so it serves as a drop-in replacement with TLS fingerprinting superpowers.
> 
> **Deliverables**:
> - Fully renamed gem (`wreq-rb`) with module `Wreq::HTTP`
> - Updated Rust deps: `wreq 6.0.0-rc.27` + `wreq-util 3.0.0-rc.9`
> - Refactored Rust extension: DRY, safe (no panics), proper error handling
> - Full http.rb API parity: chainable config, options hash, status predicates, persistent connections, response parsing
> - Comprehensive TDD test suite (GOOSGBT)
> - Updated CI, benchmarks, README
> 
> **Estimated Effort**: XL
> **Parallel Execution**: YES — 3 waves
> **Critical Path**: Task 1 → Task 2 → Task 3 → Task 4 → Tasks 5-9 (parallel) → Task 10 → Task 11

---

## Context

### Original Request
"Update this project's upstream underlying library. Update and fix the current project. The goal is to use it as simple as possible, as Ruby way as possible, and as close to 100% success rate and blazing fast and safe, as possible."

### Interview Summary
**Key Discussions**:
- **Rename**: Full rename from `rquest-rb` to `wreq-rb`, module `Rquest::HTTP` → `Wreq::HTTP`
- **Upstream**: Pin to `wreq 6.0.0-rc.27` + `wreq-util 3.0.0-rc.9` (latest RC, actively maintained)
- **Ruby API**: Full http.rb (httprb/http) API parity — drop-in replacement. User has used http.rb in production for years. Includes persistent connections.
- **Refactoring**: Full cleanup — DRY up duplicated methods, proper error handling, fix expensive clone, typed errors
- **Test strategy**: TDD, GOOSGBT (Growing Object-Oriented Software Guided By Tests)
- **Constraints**: No premature abstractions/optimizations. High concurrency environment. Tests must be AI-verifiable and human-readable.

**Research Findings**:
- Upstream `rquest` renamed to `wreq` by same author (0x676e67). GitHub repo redirects.
- Current `lib.rs` has 6 HTTP methods with ~50 lines each of identical boilerplate — classic DRY violation
- 6 `expect()` panics that crash Ruby process instead of raising exceptions
- Silent error swallowing on response body read (`Err → None`)
- `ClientWrap::clone()` rebuilds entire TLS client from scratch — expensive
- Thread-local Tokio runtime pattern is safe for Ruby's GVL but each thread creates own runtime
- http.rb's `Chainable` module defines 9 HTTP verbs, 13 chainable config methods, rich `Response` with `Status` predicates and `Body` streaming

### Metis Review
**Identified Gaps** (addressed):
- **Phasing risk**: Large scope combining migration + feature expansion. Addressed by organizing TODOs in dependency waves — rename/migration first, then features.
- **Backward compatibility**: No deprecated aliases needed — this is a personal/OSS project, clean break is appropriate.
- **Persistent connections**: Use wreq's native connection pooling (via hyper/tower) rather than implementing Ruby-side connection manager. Simpler, aligns with "simple as possible".
- **Test focus**: Ruby tests (Minitest) are primary. Rust tests are secondary — most were already skipped due to Ruby thread context issues.
- **BoringSSL cross-platform**: wreq uses BoringSSL. CI needs `cmake`, `perl`, `libclang-dev` for Linux builds. Added to CI update task.
- **DRY vs "no premature abstractions"**: Extracting shared request logic is NOT abstraction — it's eliminating copy-paste duplication. Required to add http.rb features without multiplying duplication.
- **Missing file paths**: Added `build.rs`, `.cargo/config.toml`, `script/*.rb`, benchmark Rust files to rename scope.

---

## Work Objectives

### Core Objective
Migrate from deprecated `rquest` to `wreq` upstream, rename the gem to `wreq-rb`, and make it a production-ready drop-in replacement for `http.rb` with TLS fingerprinting capabilities for high-concurrency environments.

### Concrete Deliverables
- Renamed gem: `wreq-rb` with `Wreq::HTTP` module
- Updated Cargo deps: `wreq = "6.0.0-rc.27"`, `wreq-util = "3.0.0-rc.9"`
- Refactored `ext/wreq_rb/src/lib.rs`: DRY, safe, no panics, typed errors
- Full http.rb chainable API: `.timeout()`, `.cookies()`, `.basic_auth()`, `.accept()`, `.via()`, `.persistent()`, `.auth()`, `.follow()`, `.headers()`, `.encoding()`
- Options hash support: `:body`, `:json`, `:form`, `:params`
- Rich response object: `Status` with predicates, `Body` with streaming, `.parse`, `.flush`
- Generic `.request(verb, uri, options)` method
- TDD test suite with Minitest
- Updated CI workflows, benchmarks, README

### Definition of Done
- [ ] `ruby -e "require 'wreq-rb'; puts Wreq::HTTP::VERSION"` outputs version without error
- [ ] `bundle exec rake test` passes (all Ruby tests green)
- [ ] `grep -r "rquest" --include="*.rs" --include="*.rb" --include="*.toml" --include="*.gemspec" ext/ lib/ test/ Rakefile Cargo.toml` returns NO matches (except maybe comments explaining migration)
- [ ] `bundle exec rake compile` succeeds
- [ ] All http.rb chainable methods work: `.timeout()`, `.cookies()`, `.basic_auth()`, `.accept()`, `.persistent()`, `.via()`, `.follow()`, `.headers()`
- [ ] Response `.parse` auto-parses JSON
- [ ] Response `.status.success?`, `.status.ok?`, `.status.redirect?` work
- [ ] Persistent connections reuse underlying wreq `Client`
- [ ] No `expect()` panics in production Rust code

### Must Have
- All 6 HTTP verbs: GET, POST, PUT, DELETE, HEAD, PATCH
- Chainable configuration: `.headers()`, `.timeout()`, `.follow()`, `.cookies()`, `.basic_auth()`, `.auth()`, `.accept()`, `.via()`, `.persistent()`, `.encoding()`
- Options hash: `:body`, `:json`, `:form`, `:params`
- Response: status predicates, `.parse`, `.content_type`, `.charset`, `.uri`, `.code`, `.flush`
- TLS fingerprinting: `.desktop`, `.mobile`, random emulation
- Thread safety for high concurrency
- TDD tests for every feature

### Must NOT Have (Guardrails)
- **No premature abstractions**: Don't create RequestBuilder → Request → RequestExecutor chains. One helper function for shared logic, that's it.
- **No premature optimizations**: Don't add connection pooling beyond what wreq provides natively.
- **No `expect()` panics**: Every Rust error must map to a Ruby exception, never crash the process.
- **No silent error swallowing**: Body read errors must be surfaced, not hidden.
- **No over-engineering the response body**: Start with String body. Add streaming (`.each`, `.readpartial`) as methods on the response, don't create a separate Body class unless http.rb compat absolutely requires it.
- **No deprecated aliases**: No `Rquest::HTTP` alias pointing to `Wreq::HTTP`. Clean break.
- **No `.trace`, `.options`, `.connect` verbs**: Not in scope for this plan.
- **No `.retriable()` or `.use()` features**: Not in scope for this plan.
- **No `.nodelay`**: Not in scope — wreq handles TCP options internally.

---

## Verification Strategy (MANDATORY)

> **UNIVERSAL RULE: ZERO HUMAN INTERVENTION**
>
> ALL tasks in this plan MUST be verifiable WITHOUT any human action.
> This is NOT conditional — it applies to EVERY task, regardless of test strategy.
>
> **FORBIDDEN** — acceptance criteria that require:
> - "User manually tests..." 
> - "User visually confirms..."
> - "User interacts with..."
> - "Ask user to verify..."
> - ANY step where a human must perform an action
>
> **ALL verification is executed by the agent** using tools (Bash, interactive_bash, etc.). No exceptions.

### Test Decision
- **Infrastructure exists**: YES (Minitest in `test/rquest_test.rb`, Rake tasks)
- **Automated tests**: TDD (GOOSGBT — Growing Object-Oriented Software Guided By Tests)
- **Framework**: Minitest (existing), Rust tests via `cargo test`

### TDD Workflow Per Feature

Each feature follows RED-GREEN-REFACTOR:

1. **RED**: Write failing test first in `test/wreq_test.rb`
   - Test command: `bundle exec rake ruby_test`
   - Expected: FAIL (test exists, feature doesn't)
2. **GREEN**: Implement minimum Rust/Ruby code to pass
   - Command: `bundle exec rake ruby_test`
   - Expected: PASS
3. **REFACTOR**: Clean up while keeping green
   - Command: `bundle exec rake ruby_test`
   - Expected: PASS (still)

### Agent-Executed QA Scenarios (MANDATORY — ALL tasks)

Every task includes QA scenarios that the executing agent runs directly.

**Verification Tool by Deliverable Type:**

| Type | Tool | How Agent Verifies |
|------|------|-------------------|
| **Rust compilation** | Bash | `bundle exec rake compile` → exit 0 |
| **Ruby tests** | Bash | `bundle exec rake ruby_test` → exit 0, specific test counts |
| **Rename completeness** | Bash (grep) | `grep -r "rquest" ...` → no matches |
| **API behavior** | Bash (ruby -e) | Inline Ruby snippets asserting behavior |
| **Live HTTP** | Bash (ruby -e) | Requests to httpbin.org/tls.peet.ws |

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Foundation — Sequential, must complete first):
├── Task 1: Rename all files and references (rquest → wreq)
├── Task 2: Update Cargo deps + fix Rust imports to compile
└── Task 3: Refactor Rust core — DRY + safety fixes

Wave 2 (http.rb Parity Features — Parallel):
├── Task 4: Options hash (:body, :json, :form, :params) + generic .request()
├── Task 5: Chainable .timeout() 
├── Task 6: Response Status object with predicates
├── Task 7: Response .parse + enhanced body
├── Task 8: Chainable .cookies(), .basic_auth(), .auth(), .accept()
├── Task 9: Chainable .via() (http.rb-style proxy)
├── Task 10: .persistent() with block form
└── Task 11: .follow() with options hash (max_hops)

Wave 3 (Polish — Sequential after Wave 2):
├── Task 12: .encoding() + .flush() + remaining response methods
├── Task 13: Update benchmarks, README, gemspec, CI workflows
└── Task 14: Final integration test suite + smoke test
```

### Dependency Matrix

| Task | Depends On | Blocks | Can Parallelize With |
|------|------------|--------|---------------------|
| 1 | None | 2 | None |
| 2 | 1 | 3 | None |
| 3 | 2 | 4-11 | None |
| 4 | 3 | 12, 14 | 5, 6, 7, 8, 9, 10, 11 |
| 5 | 3 | 14 | 4, 6, 7, 8, 9, 10, 11 |
| 6 | 3 | 7, 14 | 4, 5, 8, 9, 10, 11 |
| 7 | 3, 6 | 14 | 4, 5, 8, 9, 10, 11 |
| 8 | 3 | 14 | 4, 5, 6, 7, 9, 10, 11 |
| 9 | 3 | 14 | 4, 5, 6, 7, 8, 10, 11 |
| 10 | 3 | 14 | 4, 5, 6, 7, 8, 9, 11 |
| 11 | 3 | 14 | 4, 5, 6, 7, 8, 9, 10 |
| 12 | 4, 6, 7 | 14 | 13 |
| 13 | 3 | 14 | 12 |
| 14 | ALL | None | None (final) |

### Agent Dispatch Summary

| Wave | Tasks | Recommended Agents |
|------|-------|-------------------|
| 1 | 1, 2, 3 | Sequential: `task(category="deep", ...)` — each builds on prior |
| 2 | 4-11 | Parallel: `task(category="deep", ...)` — independent features |
| 3 | 12, 13, 14 | Sequential after Wave 2: `task(category="unspecified-high", ...)` |

---

## TODOs

- [ ] 1. Rename all files and references: rquest → wreq

  **What to do**:
  - Rename directory `ext/rquest_rb/` → `ext/wreq_rb/`
  - Rename file `lib/rquest_rb.rb` → `lib/wreq_rb.rb`
  - Rename directory `lib/rquest_rb/` → `lib/wreq_rb/`  
  - Rename file `rquest-rb.gemspec` → `wreq-rb.gemspec`
  - Rename file `test/rquest_test.rb` → `test/wreq_test.rb`
  - Update ALL file contents — every occurrence of `rquest`/`Rquest` → `wreq`/`Wreq`:
    - `ext/wreq_rb/Cargo.toml`: package name `wreq-rb`, lib name `wreq_rb`, description
    - `ext/wreq_rb/src/lib.rs`: `Rquest::HTTP` → `Wreq::HTTP` in `init()` function (lines 631-632), struct annotations `#[magnus::wrap(class = "Rquest::HTTP::...")]` → `"Wreq::HTTP::..."` (lines 122, 143, 488)
    - `ext/wreq_rb/extconf.rb`: `create_rust_makefile("rquest_rb/rquest_rb")` → `"wreq_rb/wreq_rb"`, `--package` arg → `"wreq-rb"`
    - `lib/wreq_rb.rb`: module `Rquest` → `Wreq`, require paths `rquest/` → `wreq/`, `rquest_rb` → `wreq_rb`
    - `lib/wreq_rb/version.rb`: module `Rquest` → `Wreq`, bump version to `1.0.0`
    - `wreq-rb.gemspec`: `spec.name = "wreq-rb"`, version ref `Wreq::HTTP::VERSION`, all text references
    - `Cargo.toml` (root): workspace member `ext/wreq_rb`
    - `Rakefile`: `GEMSPEC` path → `wreq-rb.gemspec`, extension task name → `wreq_rb`, `ext.lib_dir = "lib/wreq"`, `ext.ext_dir = "ext/wreq_rb"`, `config.binary_name = 'wreq_rb'`, load path refs
    - `Gemfile`: no changes needed (uses `gemspec`)
    - `test/wreq_test.rb`: `require_relative '../lib/wreq_rb'`, `HTTP = Wreq::HTTP`
    - `benchmark/http_clients_benchmark.rb`: `require "wreq_rb"`, `Wreq::HTTP.get(...)`, report name `"wreq-rb"`
    - `benchmark/http_clients_benchmark.rs`: function name, `rquest::` → `wreq::`, `rquest_util::` → `wreq_util::`
    - `benchmark/rust_profiling_benchmark.rs`: `use rquest::Client` → `use wreq::Client`
    - `.github/workflows/cross-compile.yml`: `librquest_rb` → `libwreq_rb`, `rquest_rb.gemspec` → `wreq_rb.gemspec`
    - `.github/workflows/benchmark.yml`: grep patterns for `Rquest-rb` → `Wreq-rb`, variable names
    - `script/*.rb`: check and update any `rquest` references
    - `README.md`: all occurrences of `rquest-rb`, `rquest`, `Rquest` → `wreq-rb`, `wreq`, `Wreq`

  **Must NOT do**:
  - Do NOT update Rust `use rquest::*` import paths yet — that's Task 2
  - Do NOT update Cargo dependency versions yet — that's Task 2
  - Do NOT refactor any logic — just rename

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: File renames + mass string replacements across 20+ files requires careful tracking to ensure nothing is missed
  - **Skills**: [`git-master`]
    - `git-master`: Need `git mv` for directory renames to preserve history

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1 (sequential)
  - **Blocks**: Task 2
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `ext/rquest_rb/extconf.rb:4` — Current `create_rust_makefile("rquest_rb/rquest_rb")` pattern showing how extension name maps to directory structure
  - `lib/rquest_rb.rb:10-14` — Ruby version-specific require path pattern: `require "rquest/#{version}/rquest_rb"` — must become `"wreq/#{version}/wreq_rb"`
  - `Rakefile:25-30` — `Rake::ExtensionTask.new("rquest_rb", GEMSPEC)` with `ext.lib_dir = "lib/rquest"` — shows naming convention between extension task, lib dir, and ext dir
  - `ext/rquest_rb/src/lib.rs:631-632` — `ruby.define_module("Rquest")` and `rquest_module.define_module("HTTP")` — Ruby module names defined in Rust
  - `ext/rquest_rb/src/lib.rs:122,143,488` — `#[magnus::wrap(class = "Rquest::HTTP::Client")]` and `"Rquest::HTTP::Response"` — class name strings in Rust annotations

  **File List** (every file requiring changes):
  - `ext/rquest_rb/Cargo.toml` (lines 1-9: package metadata)
  - `ext/rquest_rb/src/lib.rs` (lines 122, 143, 488, 631-632, 636-670: Ruby module/class names)
  - `ext/rquest_rb/extconf.rb` (lines 4, 6: makefile name and package)
  - `lib/rquest_rb.rb` (lines 1, 3, 11-14: module and require paths)
  - `lib/rquest_rb/version.rb` (lines 1-3: module name)
  - `rquest-rb.gemspec` (lines 1, 4-5, 9-10, 17, 24: name, version ref, URLs)
  - `Cargo.toml` (line 2: workspace member path)
  - `Rakefile` (lines 5, 25-30, 102, 153: gemspec path, extension task, memcheck binary, load paths)
  - `test/rquest_test.rb` (lines 2, 6: require and module constant)
  - `benchmark/http_clients_benchmark.rb` (lines 8, 23-24: require and usage)
  - `benchmark/http_clients_benchmark.rs` (lines 19-21, 44: function and crate names)
  - `benchmark/rust_profiling_benchmark.rs` (line 2: use statement)
  - `.github/workflows/cross-compile.yml` (line 32, 36: library name, gemspec)
  - `.github/workflows/benchmark.yml` (lines 62, 66: grep patterns for benchmark names)
  - `README.md` (throughout: all `rquest` references)

  **Acceptance Criteria**:

  - [ ] `grep -r "Rquest" --include="*.rs" --include="*.rb" --include="*.toml" --include="*.gemspec" ext/ lib/ test/ Rakefile Cargo.toml` returns NO matches
  - [ ] `grep -r "rquest" --include="*.rs" --include="*.rb" --include="*.toml" --include="*.gemspec" ext/ lib/ test/ Rakefile Cargo.toml` returns NO matches (except comments explaining migration)
  - [ ] `ls ext/wreq_rb/src/lib.rs` succeeds (directory renamed)
  - [ ] `ls lib/wreq_rb.rb` succeeds (file renamed)
  - [ ] `ls wreq-rb.gemspec` succeeds (file renamed)
  - [ ] `ls test/wreq_test.rb` succeeds (file renamed)

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: All rquest references eliminated
    Tool: Bash (grep)
    Preconditions: All renames complete
    Steps:
      1. grep -ri "rquest" --include="*.rs" --include="*.rb" --include="*.toml" --include="*.gemspec" --include="*.yml" ext/ lib/ test/ Rakefile Cargo.toml .github/
      2. Assert: Exit code 1 (no matches) OR only matches in comments explaining migration history
    Expected Result: Zero functional references to old name
    Evidence: grep output captured

  Scenario: Directory structure correct after rename
    Tool: Bash (ls/find)
    Preconditions: Renames complete
    Steps:
      1. ls ext/wreq_rb/src/lib.rs → exists
      2. ls ext/wreq_rb/Cargo.toml → exists
      3. ls ext/wreq_rb/extconf.rb → exists
      4. ls ext/wreq_rb/build.rs → exists
      5. ls lib/wreq_rb.rb → exists
      6. ls lib/wreq_rb/version.rb → exists
      7. ls wreq-rb.gemspec → exists
      8. ls test/wreq_test.rb → exists
      9. ls ext/rquest_rb/ → should NOT exist (old dir gone)
      10. ls lib/rquest_rb.rb → should NOT exist (old file gone)
    Expected Result: New paths exist, old paths don't
    Evidence: ls output captured
  ```

  **Commit**: YES
  - Message: `rename: rquest-rb → wreq-rb across entire project`
  - Files: All renamed/modified files
  - Pre-commit: `ls ext/wreq_rb/src/lib.rs && ls lib/wreq_rb.rb && ls wreq-rb.gemspec`

---

- [ ] 2. Update Cargo dependencies + fix Rust imports to compile

  **What to do**:
  - In `ext/wreq_rb/Cargo.toml`:
    - Change `rquest = "5.1.0"` → `wreq = "6.0.0-rc.27"`
    - Change `rquest-util = "2.1.0"` → `wreq-util = "3.0.0-rc.9"`
    - Keep all other deps as-is (magnus, rb-sys, tokio, etc.)
    - In dev-dependencies: `reqwest = "0.12"` can stay (used for benchmarks)
  - In `ext/wreq_rb/src/lib.rs`:
    - Change `use rquest::redirect::Policy` → `use wreq::redirect::Policy`
    - Change `use rquest::{Error as RquestError, Response as RquestResponse}` → `use wreq::{Error as WreqError, Response as WreqResponse}`
    - Change `use rquest_util::Emulation as RquestEmulation` → `use wreq_util::Emulation as WreqEmulation`
    - Update all type references: `RquestError` → `WreqError`, `RquestResponse` → `WreqResponse`, `RquestEmulation` → `WreqEmulation`
    - Update all `rquest::Client::builder()` → `wreq::Client::builder()`
    - Update struct: `struct ClientWrap(rquest::Client)` → `struct ClientWrap(wreq::Client)`
    - Update emulation variant names to match `wreq-util 3.0.0-rc.9`:
      - Verify which `Emulation` variants exist in the new version
      - Update `get_random_desktop_emulation()`, `get_random_mobile_emulation()` with current variant names
    - Update `rquest_error_to_magnus_error` → `wreq_error_to_magnus_error` (function name)
  - In Rust benchmark files:
    - `benchmark/http_clients_benchmark.rs`: `use rquest::*` → `use wreq::*`, `rquest_util::Emulation` → `wreq_util::Emulation`
    - `benchmark/rust_profiling_benchmark.rs`: `use rquest::Client` → `use wreq::Client`
  - Verify compilation: `bundle exec rake compile`

  **Must NOT do**:
  - Do NOT refactor logic — just swap imports and types
  - Do NOT change API surface
  - Do NOT fix `expect()` panics yet — that's Task 3

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Need to verify wreq API compatibility, check emulation variant names against actual crate, handle any breaking changes
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1 (sequential)
  - **Blocks**: Task 3
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/src/lib.rs:6-8` — Current import statements that need updating
  - `ext/wreq_rb/src/lib.rs:49-80` — Emulation variant lists that may need new names
  - `ext/wreq_rb/src/lib.rs:82-87` — Error conversion function to rename
  - `ext/wreq_rb/src/lib.rs:123` — `struct ClientWrap(rquest::Client)` type reference

  **External References**:
  - wreq crate docs: `https://docs.rs/wreq/6.0.0-rc.27` — verify `Client`, `redirect::Policy`, `Error`, `Response` exist
  - wreq-util crate docs: `https://docs.rs/wreq-util/3.0.0-rc.9` — verify `Emulation` variants (e.g., `Safari26`, `Chrome134`, `Firefox135`)
  - wreq GitHub: `https://github.com/0x676e67/wreq` — README examples show usage patterns

  **Acceptance Criteria**:

  - [ ] `bundle exec rake compile` succeeds (exit code 0)
  - [ ] `grep -r "use rquest" ext/wreq_rb/src/lib.rs` returns NO matches
  - [ ] `grep -r "RquestError\|RquestResponse\|RquestEmulation" ext/wreq_rb/src/lib.rs` returns NO matches
  - [ ] Existing Ruby tests still pass: `bundle exec rake ruby_test` (exit code 0)

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: Rust extension compiles with wreq deps
    Tool: Bash
    Preconditions: Task 1 complete, directory structure renamed
    Steps:
      1. bundle exec rake compile
      2. Assert: exit code 0
      3. Assert: No compilation errors in output
    Expected Result: Native extension builds successfully
    Evidence: Compilation output captured

  Scenario: Existing tests pass after dep swap
    Tool: Bash
    Preconditions: Extension compiled
    Steps:
      1. bundle exec rake ruby_test
      2. Assert: exit code 0
      3. Assert: Output contains "0 failures, 0 errors"
    Expected Result: All existing tests green
    Evidence: Test output captured

  Scenario: Basic HTTP request works with wreq backend
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.get('https://httpbin.org/get'); puts r.status; exit(r.status == 200 ? 0 : 1)"
      2. Assert: exit code 0
      3. Assert: stdout contains "200"
    Expected Result: GET request succeeds with wreq backend
    Evidence: Script output captured
  ```

  **Commit**: YES
  - Message: `deps: migrate from rquest 5.1.0 to wreq 6.0.0-rc.27`
  - Files: `ext/wreq_rb/Cargo.toml`, `ext/wreq_rb/src/lib.rs`, `benchmark/*.rs`
  - Pre-commit: `bundle exec rake compile && bundle exec rake ruby_test`

---

- [ ] 3. Refactor Rust core — DRY + safety fixes

  **What to do**:

  **3a. Replace all `expect()` panics with proper Result handling**:
  - `get_runtime()` (line ~97): `Runtime::new().expect(...)` → Return `Result<Arc<Runtime>, MagnusError>`, propagate error
  - `RbHttpClient::new()` (line ~159): `builder().build().expect(...)` → Return `Result<Self, MagnusError>`
  - `RbHttpClient::new_desktop()` (line ~174): Same pattern
  - `RbHttpClient::new_mobile()` (line ~189): Same pattern
  - `ClientWrap::clone()` (line ~138): `builder().build().expect(...)` → Handle properly
  - `with_proxy()` (line ~219): `builder().proxy().build().expect(...)` → Handle properly
  - Every constructor that can fail must return `Result` to Ruby, not panic

  **3b. Fix silent error swallowing**:
  - `RbHttpResponse::new()` (lines ~507-511): `response.text().await` → `Err(_) => None` should either:
    - Store the error and surface it when `.body()` is called
    - OR return empty string with a way to check if body read failed
    - Prefer: map to empty string and log a warning (no silent failure)

  **3c. DRY up HTTP methods — extract shared `execute_request` helper**:
  - Create an enum or parameter for HTTP method: `Get, Post, Put, Delete, Head, Patch`
  - Create ONE private method `execute_request(&self, method, url, body, options) -> Result<RbHttpResponse, MagnusError>` that handles:
    1. Get runtime
    2. Build request from client with correct HTTP method
    3. Apply default headers
    4. Apply Accept default (if not set)
    5. Apply User-Agent (if set)
    6. Configure redirect policy
    7. Apply timeout
    8. Set body (if provided)
    9. Execute with `block_on` and map errors
  - Each public method (get, post, etc.) becomes a 1-3 line wrapper calling `execute_request`

  **3d. Fix ClientWrap::clone() — don't rebuild TLS client**:
  - wreq's `Client` implements `Clone` internally (it shares the connection pool via Arc)
  - `ClientWrap::clone()` should simply clone the inner `wreq::Client`, NOT call `builder().build()`
  - This is both faster and correct — cloned clients share the connection pool

  **3e. Prepare for http.rb features** (scaffolding only):
  - Add fields to `RbHttpClient` for future features: `cookies: Option<HashMap<String, String>>`, `auth_header: Option<String>`, `accept_type: Option<String>`
  - These fields are set by chainable methods but not yet wired into `execute_request`
  - The `execute_request` helper should accept an options struct for extensibility

  **Must NOT do**:
  - Do NOT add new Ruby-facing API methods — that's Tasks 4-11
  - Do NOT change test expectations — tests should still pass as-is
  - Do NOT add abstraction layers (no RequestBuilder, no middleware chain)
  - Keep it flat: struct + methods, nothing more

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Core Rust refactoring with safety implications, need to ensure no regressions, touch every method in the file
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1 (sequential)
  - **Blocks**: Tasks 4-11
  - **Blocked By**: Task 2

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/src/lib.rs:231-261` — Current `get()` method (template for DRY extraction — has header setup, redirect, timeout, send, error mapping)
  - `ext/wreq_rb/src/lib.rs:263-303` — Current `post()` method (same as get + body + content-type default)
  - `ext/wreq_rb/src/lib.rs:89-101` — Current `get_runtime()` with `expect()` panic
  - `ext/wreq_rb/src/lib.rs:131-141` — Current `ClientWrap::clone()` that rebuilds TLS client
  - `ext/wreq_rb/src/lib.rs:493-522` — Current `RbHttpResponse::new()` with silent error swallowing
  - `ext/wreq_rb/src/lib.rs:82-87` — Current error conversion function (pattern to extend)

  **External References**:
  - wreq `Client::clone()` behavior: wreq inherits from reqwest — `Client` uses `Arc<ClientRef>` internally, clone is cheap and shares connection pool
  - magnus error handling: `magnus::Error::new(exception::runtime_error(), msg)` — pattern for all error conversions

  **Acceptance Criteria**:

  - [ ] `grep -n "expect(" ext/wreq_rb/src/lib.rs` returns NO matches in production code (only in `#[cfg(test)]` blocks)
  - [ ] `bundle exec rake compile` succeeds
  - [ ] `bundle exec rake ruby_test` passes (all existing tests green)
  - [ ] Line count of `ext/wreq_rb/src/lib.rs` decreased (was 787 lines, should be ~500-600 after DRY)
  - [ ] `cargo test -- --test-threads=1` passes for non-skipped tests

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: No expect() panics in production code
    Tool: Bash (grep)
    Preconditions: Refactoring complete
    Steps:
      1. grep -n "expect(" ext/wreq_rb/src/lib.rs
      2. For each match: verify it's inside a #[cfg(test)] block
      3. Assert: Zero expect() calls in non-test code
    Expected Result: All panics eliminated from production paths
    Evidence: grep output captured

  Scenario: Clone doesn't rebuild TLS client
    Tool: Bash (grep)
    Preconditions: Refactoring complete
    Steps:
      1. grep -A5 "impl Clone for ClientWrap" ext/wreq_rb/src/lib.rs
      2. Assert: clone body does NOT contain "builder()" or "build()"
      3. Assert: clone body contains simple field clone (e.g., "self.0.clone()")
    Expected Result: Clone is cheap, shares connection pool
    Evidence: grep output captured

  Scenario: DRY refactor — single execute_request helper exists
    Tool: Bash (grep)
    Preconditions: Refactoring complete
    Steps:
      1. grep -n "fn execute_request\|fn send_request\|fn do_request" ext/wreq_rb/src/lib.rs
      2. Assert: Exactly 1 match for the shared helper function
      3. wc -l ext/wreq_rb/src/lib.rs
      4. Assert: Line count < 650 (was 787)
    Expected Result: Duplication eliminated, code is shorter
    Evidence: Line count and grep output captured

  Scenario: Error handling works — invalid URL raises Ruby exception
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; begin; Wreq::HTTP.get('not-a-valid-url'); rescue RuntimeError => e; puts e.message; exit 0; end; exit 1"
      2. Assert: exit code 0
      3. Assert: stdout contains error message (not a crash/segfault)
    Expected Result: Invalid URL raises RuntimeError, doesn't crash process
    Evidence: Script output captured

  Scenario: All existing tests still pass after refactor
    Tool: Bash
    Preconditions: Refactoring complete
    Steps:
      1. bundle exec rake ruby_test
      2. Assert: exit code 0
      3. Assert: Same test count as before (13 tests, 0 failures)
    Expected Result: Zero regressions
    Evidence: Test output captured
  ```

  **Commit**: YES
  - Message: `refactor: DRY up HTTP methods, eliminate expect() panics, fix clone and error handling`
  - Files: `ext/wreq_rb/src/lib.rs`
  - Pre-commit: `bundle exec rake compile && bundle exec rake ruby_test`

---

- [ ] 4. Options hash support (:body, :json, :form, :params) + generic .request()

  **What to do**:

  **TDD — Write tests first (RED)**:
  - Test: `HTTP.post(url, json: { name: "test" })` — auto-serializes to JSON, sets Content-Type
  - Test: `HTTP.post(url, form: { name: "test" })` — URL-encodes, sets Content-Type
  - Test: `HTTP.post(url, body: "raw string")` — raw body (existing, verify still works)
  - Test: `HTTP.get(url, params: { q: "search" })` — appends query string to URL
  - Test: `HTTP.request(:get, url)` — generic request method
  - Test: `HTTP.request(:post, url, json: { a: 1 })` — generic with options
  - Test: options hash keys are symbols (Ruby convention)

  **Implement (GREEN)**:
  - Update `extract_body` (or replace it) to parse options hash with `:body`, `:json`, `:form`, `:params` keys
  - For `:json`: serialize with `serde_json::to_string()`, set `Content-Type: application/json`
  - For `:form`: URL-encode key=value pairs, set `Content-Type: application/x-www-form-urlencoded`
  - For `:body`: pass through as-is (existing behavior)
  - For `:params`: append to URL as query string (use `url` crate already in deps)
  - Add `request(verb, url, options)` method to `RbHttpClient` — dispatches to `execute_request` with the right HTTP method
  - Register `request` as Ruby method on both module and client class
  - All HTTP verb methods should accept options hash: `get(url, **options)`, `post(url, **options)`, etc.

  **Must NOT do**:
  - Do NOT create a separate Options class or builder — use Ruby hash directly
  - Do NOT change the signature of existing methods in a breaking way — `:body` key should still work exactly as before

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Involves Rust-Ruby FFI argument parsing (magnus Value/RHash), serialization logic, URL manipulation
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 5-11)
  - **Blocks**: Task 12, 14
  - **Blocked By**: Task 3

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/src/lib.rs:103-120` — Current `extract_body()` function showing RHash parsing with Symbol keys
  - `ext/wreq_rb/src/lib.rs:592-615` — Current module-level `rb_post/rb_put/rb_patch` functions using variadic args
  - `ext/wreq_rb/Cargo.toml:27` — `serde_json = "1.0"` already in deps (for JSON serialization)
  - `ext/wreq_rb/Cargo.toml:26` — `url = "2.5"` already in deps (for query string building)

  **External References**:
  - http.rb options: `HTTP.post(url, json: {foo: 42})` — from https://github.com/httprb/http/wiki/Passing-Parameters
  - http.rb params: `HTTP.get(url, params: {q: "search"})` — query string parameters
  - magnus RHash API: iterate with `foreach`, get with Symbol keys

  **Acceptance Criteria**:

  - [ ] TDD: Tests written BEFORE implementation, initially RED
  - [ ] `bundle exec rake ruby_test` passes with all new tests GREEN
  - [ ] `HTTP.post("https://httpbin.org/post", json: { name: "test" })` returns 200 with JSON body echoed
  - [ ] `HTTP.post("https://httpbin.org/post", form: { name: "test" })` returns 200 with form data echoed
  - [ ] `HTTP.get("https://httpbin.org/get", params: { q: "search" })` returns 200 with `args.q == "search"`
  - [ ] `HTTP.request(:get, "https://httpbin.org/get")` returns 200
  - [ ] Existing `:body` tests still pass (backward compat)

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: JSON body auto-serialization
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled, feature implemented
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.post('https://httpbin.org/post', json: { name: 'test', value: 123 }); body = JSON.parse(r.body); puts body['json']['name']; exit(body['json']['name'] == 'test' ? 0 : 1)"
      2. Assert: exit code 0
      3. Assert: stdout contains "test"
    Expected Result: JSON body correctly serialized and echoed back
    Evidence: Script output captured

  Scenario: Form body URL-encoding
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled, feature implemented
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.post('https://httpbin.org/post', form: { name: 'test', email: 'a@b.com' }); body = JSON.parse(r.body); puts body['form']['name']; exit(body['form']['name'] == 'test' ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: Form data correctly URL-encoded
    Evidence: Script output captured

  Scenario: Query params appended to URL
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled, feature implemented
    Steps:
      1. ruby -e "require 'wreq-rb'; require 'json'; r = Wreq::HTTP.get('https://httpbin.org/get', params: { q: 'search', page: '2' }); body = JSON.parse(r.body); puts body['args']['q']; exit(body['args']['q'] == 'search' && body['args']['page'] == '2' ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: Query parameters visible in httpbin response
    Evidence: Script output captured

  Scenario: Generic .request method works
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled, feature implemented
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.request(:get, 'https://httpbin.org/get'); exit(r.status == 200 ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: Generic request method dispatches correctly
    Evidence: Script output captured
  ```

  **Commit**: YES (groups with Task 5-11 if done in same wave)
  - Message: `feat: add options hash support (:json, :form, :params) and generic .request() method`
  - Files: `ext/wreq_rb/src/lib.rs`, `test/wreq_test.rb`
  - Pre-commit: `bundle exec rake ruby_test`

---

- [x] 5. Chainable .timeout()

  **What to do**:

  **TDD — Write tests first (RED)**:
  - Test: `HTTP.timeout(5).get(url)` — global timeout of 5 seconds
  - Test: `HTTP.timeout(connect: 5, read: 10).get(url)` — per-operation timeouts
  - Test: `HTTP.timeout(0.001).get(url)` — very short timeout raises error
  - Test: chainable with other methods: `HTTP.headers(...).timeout(5).get(url)`

  **Implement (GREEN)**:
  - Add `timeout` chainable method to `RbHttpClient` that accepts either:
    - A numeric value (global timeout in seconds)
    - A hash with `:connect`, `:read`, `:write` keys (per-operation timeouts)
  - Store timeout config in `RbHttpClient` fields
  - Wire into `execute_request` helper — apply timeout to wreq request builder
  - Register as Ruby method on both module and client class
  - wreq supports `.timeout(Duration)` on request builder — use this for global timeout
  - For per-operation: wreq's `ClientBuilder` has `.connect_timeout()` — may need to build a new client for connect timeout. Read timeout can be set per-request.

  **Must NOT do**:
  - Do NOT implement `HTTP::Timeout::Null` or `HTTP::Timeout::Global` classes — just use numeric/hash

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Single feature addition, well-scoped, pattern established by Task 3's execute_request
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4, 6-11)
  - **Blocks**: Task 14
  - **Blocked By**: Task 3

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/src/lib.rs:149` — Current `timeout: Option<Duration>` field on RbHttpClient
  - `ext/wreq_rb/src/lib.rs:253-255` — Current timeout application pattern in request methods

  **External References**:
  - http.rb timeout API: `HTTP.timeout(5)` or `HTTP.timeout(connect: 5, read: 10, write: 10)` — from chainable.rb lines 91-109
  - wreq/reqwest timeout: `RequestBuilder::timeout(Duration)` for per-request, `ClientBuilder::connect_timeout(Duration)` for connect

  **Acceptance Criteria**:

  - [ ] TDD: Tests written BEFORE implementation, initially RED
  - [ ] `HTTP.timeout(30).get("https://httpbin.org/get")` returns 200
  - [ ] `HTTP.timeout(connect: 5, read: 30).get("https://httpbin.org/get")` returns 200
  - [ ] `HTTP.timeout(0.001).get("https://httpbin.org/delay/5")` raises RuntimeError (timeout)
  - [ ] Chainable: `HTTP.headers(accept: "application/json").timeout(10).get(url)` works

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: Global timeout works
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.timeout(30).get('https://httpbin.org/get'); exit(r.status == 200 ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: Request succeeds within timeout
    Evidence: Script output captured

  Scenario: Timeout triggers on slow response
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; begin; Wreq::HTTP.timeout(1).get('https://httpbin.org/delay/10'); exit 1; rescue RuntimeError => e; puts e.message; exit 0; end"
      2. Assert: exit code 0
      3. Assert: stdout contains timeout-related error
    Expected Result: Timeout error raised, not hung
    Evidence: Script output captured
  ```

  **Commit**: YES
  - Message: `feat: add chainable .timeout() with global and per-operation support`
  - Files: `ext/wreq_rb/src/lib.rs`, `test/wreq_test.rb`
  - Pre-commit: `bundle exec rake ruby_test`

---

- [x] 6. Response Status object with predicates

  **What to do**:

  **TDD — Write tests first (RED)**:
  - Test: `response.status` returns an object (not just integer)
  - Test: `response.status == 200` still works (integer coercion / `==` operator)
  - Test: `response.status.success?` returns true for 2xx
  - Test: `response.status.ok?` returns true for exactly 200
  - Test: `response.status.redirect?` returns true for 3xx
  - Test: `response.status.client_error?` returns true for 4xx
  - Test: `response.status.server_error?` returns true for 5xx
  - Test: `response.status.informational?` returns true for 1xx
  - Test: `response.status.reason` returns "OK", "Not Found", etc.
  - Test: `response.status.to_s` returns "200 OK"
  - Test: `response.status.to_i` returns integer
  - Test: `response.code` still returns integer (backward compat)

  **Implement (GREEN)**:
  - Create a new `RbHttpStatus` struct in Rust wrapping a u16 status code
  - Register as `Wreq::HTTP::Response::Status` class
  - Implement predicate methods: `success?` (200-299), `redirect?` (300-399), `client_error?` (400-499), `server_error?` (500-599), `informational?` (100-199)
  - Implement `ok?` as alias for status == 200
  - Implement `reason` — map common status codes to reason phrases (use a match statement or small lookup table)
  - Implement `to_s` → "200 OK"
  - Implement `to_i` → raw integer
  - Implement `==` with Fixnum comparison (magnus `define_method` for `==` that accepts Value, tries integer conversion)
  - Change `RbHttpResponse::status()` to return `RbHttpStatus` instead of `u16`
  - Keep `RbHttpResponse::code()` returning `u16` for backward compat

  **Must NOT do**:
  - Do NOT create a hierarchy of status classes — one struct with predicate methods
  - Do NOT break `response.code` — it must still return integer

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Creating a new Ruby class in Rust with magnus, implementing comparison operators, mapping status codes
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4, 5, 7-11)
  - **Blocks**: Task 7 (response.parse depends on response object), Task 14
  - **Blocked By**: Task 3

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/src/lib.rs:488-491` — Current `RbHttpResponse` with `#[magnus::wrap]` — pattern for creating new wrapped class
  - `ext/wreq_rb/src/lib.rs:635-643` — Current response class registration pattern (define_method for each method)
  - `ext/wreq_rb/src/lib.rs:524-527` — Current `status()` returning u16

  **External References**:
  - http.rb Status: `response.status.success?`, `.reason`, `.to_s` — from wiki Response-Handling
  - HTTP status reason phrases: RFC 9110 standard reason phrases

  **Acceptance Criteria**:

  - [ ] TDD: Tests written BEFORE implementation, initially RED
  - [ ] `response.status == 200` evaluates to true (integer comparison)
  - [ ] `response.status.success?` returns true for 200
  - [ ] `response.status.reason` returns "OK" for 200
  - [ ] `response.status.to_s` returns "200 OK"
  - [ ] `response.status.to_i` returns 200
  - [ ] `response.code` still returns integer (backward compat)
  - [ ] 404 response: `.client_error?` true, `.success?` false

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: Status predicates work correctly
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "
        require 'wreq-rb'
        r = Wreq::HTTP.get('https://httpbin.org/get')
        raise 'not success' unless r.status.success?
        raise 'not ok' unless r.status.ok?
        raise 'should not be redirect' if r.status.redirect?
        raise 'reason wrong' unless r.status.reason == 'OK'
        raise 'to_s wrong' unless r.status.to_s == '200 OK'
        raise 'to_i wrong' unless r.status.to_i == 200
        raise 'code wrong' unless r.code == 200
        raise 'equality broken' unless r.status == 200
        puts 'all good'
        "
      2. Assert: exit code 0, stdout contains "all good"
    Expected Result: All status predicate methods work
    Evidence: Script output captured

  Scenario: 404 status predicates
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "
        require 'wreq-rb'
        r = Wreq::HTTP.get('https://httpbin.org/status/404')
        raise 'should be client_error' unless r.status.client_error?
        raise 'should not be success' if r.status.success?
        raise 'reason wrong' unless r.status.reason == 'Not Found'
        puts 'all good'
        "
      2. Assert: exit code 0
    Expected Result: 404 correctly identified as client error
    Evidence: Script output captured
  ```

  **Commit**: YES
  - Message: `feat: add Status object with predicates (success?, redirect?, reason, to_s)`
  - Files: `ext/wreq_rb/src/lib.rs`, `test/wreq_test.rb`
  - Pre-commit: `bundle exec rake ruby_test`

---

- [x] 7. Response .parse + enhanced body handling

  **What to do**:

  **TDD — Write tests first (RED)**:
  - Test: `response.parse` auto-parses JSON when content_type is application/json
  - Test: `response.parse` returns parsed Hash for JSON
  - Test: `response.parse` returns body string when content_type is not JSON
  - Test: `response.body` still returns String (backward compat)
  - Test: `response.to_s` returns body as string
  - Test: `response.flush` returns self (for persistent connection support)

  **Implement (GREEN)**:
  - Add `parse` method to `RbHttpResponse`:
    - Check `content_type` — if contains `application/json`, parse with Ruby's `JSON.parse` (via magnus Ruby eval or call)
    - Otherwise return body as string
    - This can be done in Ruby layer OR in Rust (prefer Ruby layer for simplicity)
  - Add `flush` method to `RbHttpResponse` — reads and discards body (for persistent connections). Since we eagerly read the body already, this is essentially a no-op that returns self.
  - Consider adding `parse` in a Ruby shim file rather than Rust — this follows http.rb's pattern and avoids complex Rust↔Ruby JSON interop

  **Must NOT do**:
  - Do NOT create a separate Body class — keep response.body returning String
  - Do NOT add streaming support yet (that's P1, out of scope)
  - Do NOT implement custom MIME type parsers — just JSON for now, return String for everything else

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Small feature, can be partially implemented in Ruby layer
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4-6, 8-11)
  - **Blocks**: Task 14
  - **Blocked By**: Task 3, Task 6 (needs Status object for response enhancements)

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/src/lib.rs:528-533` — Current `body()` method returning String
  - `ext/wreq_rb/src/lib.rs:543-545` — Current `content_type()` method
  - `lib/wreq_rb.rb` — Ruby shim where `.parse` could live as a Ruby method

  **External References**:
  - http.rb `.parse`: Parses body based on content_type MIME — returns Ruby hash for JSON
  - http.rb `.flush`: Reads and discards response body, useful for persistent connections

  **Acceptance Criteria**:

  - [ ] TDD: Tests written BEFORE implementation
  - [ ] `HTTP.get("https://httpbin.org/get").parse` returns a Hash
  - [ ] `HTTP.get("https://httpbin.org/html").parse` returns a String (not JSON)
  - [ ] `response.body` still returns String (backward compat)
  - [ ] `response.flush` returns the response object (chainable)

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: Response.parse auto-parses JSON
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.get('https://httpbin.org/get'); parsed = r.parse; exit(parsed.is_a?(Hash) && parsed.key?('url') ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: JSON response auto-parsed to Hash
    Evidence: Script output captured

  Scenario: Response.parse returns string for non-JSON
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.get('https://httpbin.org/html'); parsed = r.parse; exit(parsed.is_a?(String) ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: Non-JSON body returned as string
    Evidence: Script output captured
  ```

  **Commit**: YES
  - Message: `feat: add response.parse (auto JSON) and response.flush`
  - Files: `ext/wreq_rb/src/lib.rs` or `lib/wreq_rb.rb`, `test/wreq_test.rb`
  - Pre-commit: `bundle exec rake ruby_test`

---

- [x] 8. Chainable .cookies(), .basic_auth(), .auth(), .accept()

  **What to do**:

  **TDD — Write tests first (RED)**:
  - Test: `HTTP.cookies(session: "abc123").get(url)` sends Cookie header
  - Test: `HTTP.basic_auth(user: "u", pass: "p").get(url)` sends Authorization: Basic header
  - Test: `HTTP.auth("Bearer token123").get(url)` sends Authorization header
  - Test: `HTTP.accept(:json).get(url)` sends Accept: application/json
  - Test: `HTTP.accept("text/html").get(url)` sends Accept: text/html
  - Test: chainable: `HTTP.cookies(a: "1").headers(x: "y").basic_auth(user: "u", pass: "p").get(url)`

  **Implement (GREEN)**:
  - `.cookies(hash)`: Convert hash to cookie string `"key1=value1; key2=value2"`, store as header
  - `.basic_auth(user:, pass:)`: Base64-encode `"user:pass"`, set `Authorization: Basic <encoded>`
  - `.auth(value)`: Set `Authorization: <value>` header directly
  - `.accept(type)`: If symbol (`:json`), normalize to MIME (`application/json`). If string, use directly. Set Accept header.
  - All methods return new `RbHttpClient` with updated config (immutable chain pattern, same as existing `.headers()` and `.follow()`)
  - Wire into `execute_request` — apply these headers during request building

  **Must NOT do**:
  - Do NOT implement a cookie jar (tracking cookies across requests) — just sending cookies
  - Do NOT implement bearer_auth as separate method — `.auth("Bearer token")` covers it

  **Recommended Agent Profile**:
  - **Category**: `unspecified-low`
    - Reason: These are all header-setting chainable methods — straightforward pattern, no complex logic
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4-7, 9-11)
  - **Blocks**: Task 14
  - **Blocked By**: Task 3

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/src/lib.rs:198-208` — Current `with_headers()` — pattern for all chainable methods (clone self, modify, return)
  - `ext/wreq_rb/src/lib.rs:453-466` — Current `headers()` Ruby method parsing RHash
  - `ext/wreq_rb/src/lib.rs:235-246` — Current header application in request methods

  **External References**:
  - http.rb chainable.rb:194-211 — `.basic_auth(opts)` implementation: fetch user/pass, Base64 encode, call `.auth()`
  - http.rb chainable.rb:182-184 — `.accept(type)` — wraps `.headers(Accept: MimeType.normalize(type))`
  - http.rb chainable.rb:176-178 — `.cookies(cookies)` — delegates to options

  **Acceptance Criteria**:

  - [ ] TDD: Tests written BEFORE implementation
  - [ ] `HTTP.cookies(session: "abc").get("https://httpbin.org/cookies")` returns cookies in response
  - [ ] `HTTP.basic_auth(user: "user", pass: "pass").get("https://httpbin.org/basic-auth/user/pass")` returns 200
  - [ ] `HTTP.auth("Bearer token123").get("https://httpbin.org/bearer")` returns 200
  - [ ] `HTTP.accept(:json).get(url)` sends Accept: application/json header

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: Cookies sent correctly
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; require 'json'; r = Wreq::HTTP.cookies(session: 'abc123', user: 'test').get('https://httpbin.org/cookies'); body = JSON.parse(r.body); exit(body['cookies']['session'] == 'abc123' ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: Cookies visible in httpbin response
    Evidence: Script output captured

  Scenario: Basic auth works
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.basic_auth(user: 'user', pass: 'passwd').get('https://httpbin.org/basic-auth/user/passwd'); exit(r.status == 200 ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: Basic auth accepted by httpbin
    Evidence: Script output captured
  ```

  **Commit**: YES
  - Message: `feat: add chainable .cookies(), .basic_auth(), .auth(), .accept()`
  - Files: `ext/wreq_rb/src/lib.rs`, `test/wreq_test.rb`
  - Pre-commit: `bundle exec rake ruby_test`

---

- [ ] 9. Chainable .via() (http.rb-style proxy)

  **What to do**:

  **TDD — Write tests first (RED)**:
  - Test: `HTTP.via("proxy.example.com", 8080).get(url)` — http.rb style with host + port
  - Test: `HTTP.via("proxy.example.com", 8080, "user", "pass").get(url)` — with auth
  - Test: Existing `.proxy("http://...")` still works (backward compat, can keep as alias)
  - Test: `.through` is alias for `.via`

  **Implement (GREEN)**:
  - Add `.via(*args)` method that accepts:
    - `via(host, port)` — basic proxy
    - `via(host, port, user, pass)` — proxy with auth
  - Construct proxy URL from components: `"http://user:pass@host:port"`
  - Build wreq Client with `.proxy()` on the builder
  - Note: Since wreq's proxy is set at Client-build time, `.via()` must create a new Client
  - Add `.through` as alias for `.via`
  - Register both on module and client class

  **Must NOT do**:
  - Do NOT remove existing `.proxy(url_string)` method — keep it as convenience

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Small feature, extending existing proxy support
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4-8, 10-11)
  - **Blocks**: Task 14
  - **Blocked By**: Task 3

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/src/lib.rs:210-223` — Current `with_proxy()` implementation — rebuilds client with proxy

  **External References**:
  - http.rb chainable.rb:153-168 — `.via(*proxy)` implementation with positional args

  **Acceptance Criteria**:

  - [ ] TDD: Tests written BEFORE implementation
  - [ ] `HTTP.via("proxy.example.com", 8080)` returns a client (doesn't error on construction)
  - [ ] `.through` is alias for `.via`
  - [ ] Existing `.proxy("http://...")` still works

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: via() constructs client without error
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; client = Wreq::HTTP.via('proxy.example.com', 8080); puts client.class; exit 0"
      2. Assert: exit code 0 (construction doesn't error)
    Expected Result: Client created with proxy config
    Evidence: Script output captured

  Scenario: through is alias for via
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; c1 = Wreq::HTTP.via('proxy.example.com', 8080); c2 = Wreq::HTTP.through('proxy.example.com', 8080); puts c1.class == c2.class; exit 0"
      2. Assert: exit code 0
    Expected Result: Both methods return same type
    Evidence: Script output captured
  ```

  **Commit**: YES
  - Message: `feat: add chainable .via() proxy with http.rb-compatible signature`
  - Files: `ext/wreq_rb/src/lib.rs`, `test/wreq_test.rb`
  - Pre-commit: `bundle exec rake ruby_test`

---

- [ ] 10. Persistent connections with .persistent() and block form

  **What to do**:

  **TDD — Write tests first (RED)**:
  - Test: `client = HTTP.persistent("https://httpbin.org"); r = client.get("/get"); r.status == 200`
  - Test: Block form: `HTTP.persistent("https://httpbin.org") { |http| http.get("/get") }` returns response
  - Test: Block form auto-closes: after block, client should be marked closed
  - Test: Persistent client accepts relative paths: `client.get("/get")` resolves against host
  - Test: Multiple requests through same persistent client
  - Test: `HTTP.persistent("https://httpbin.org", timeout: 30)` accepts timeout option

  **Implement (GREEN)**:
  - `.persistent(host, timeout: 5)` creates a `RbHttpClient` with a stored base URL
  - The wreq `Client` already pools connections internally (via hyper) — we leverage this
  - When a persistent client makes requests with relative URLs, prepend the base host
  - Block form: yield the client, ensure `.close` is called at the end (Ruby-side)
  - `.close` method on client — marks it as closed, future requests raise error
  - The `timeout` option sets the keep-alive timeout (map to wreq's pool idle timeout if available, otherwise store and document)
  - Implementation split: Rust for client state, Ruby shim for block/ensure pattern
  - Register on module level: `HTTP.persistent(host)` and `HTTP.persistent(host) { |http| ... }`

  **Must NOT do**:
  - Do NOT implement custom connection pooling — use wreq's built-in pooling
  - Do NOT create a separate PersistentClient class — extend RbHttpClient with `base_url` and `closed` fields

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Involves Rust + Ruby cooperation (block form in Ruby, state in Rust), relative URL resolution, lifecycle management
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4-9, 11)
  - **Blocks**: Task 14
  - **Blocked By**: Task 3

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/Cargo.toml:26` — `url = "2.5"` already in deps — use for URL resolution
  - `ext/wreq_rb/src/lib.rs:144-150` — Current `RbHttpClient` fields — add `base_url: Option<String>`, `closed: bool`

  **External References**:
  - http.rb chainable.rb:123-147 — `.persistent(host, timeout: 5)` — block form with auto-close
  - http.rb spec: `HTTP.persistent(host)` returns `HTTP::Client`, `.persistent?` returns true
  - wreq Client: Internally uses connection pooling via hyper, `Client::clone()` shares the pool

  **Acceptance Criteria**:

  - [ ] TDD: Tests written BEFORE implementation
  - [ ] `HTTP.persistent("https://httpbin.org").get("/get").status == 200`
  - [ ] Block form returns result of last expression
  - [ ] Multiple requests through same client reuse connection (verified by performance or header inspection)
  - [ ] Relative URLs resolve correctly against base host
  - [ ] Client responds to `.close` and further requests after close raise error

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: Persistent client with relative URLs
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "
        require 'wreq-rb'
        client = Wreq::HTTP.persistent('https://httpbin.org')
        r1 = client.get('/get')
        r2 = client.get('/ip')
        puts r1.status
        puts r2.status
        exit(r1.status == 200 && r2.status == 200 ? 0 : 1)
        "
      2. Assert: exit code 0
    Expected Result: Both requests succeed through persistent client
    Evidence: Script output captured

  Scenario: Block form with auto-close
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "
        require 'wreq-rb'
        result = Wreq::HTTP.persistent('https://httpbin.org') do |http|
          http.get('/get')
        end
        exit(result.status == 200 ? 0 : 1)
        "
      2. Assert: exit code 0
    Expected Result: Block form works, returns last expression
    Evidence: Script output captured
  ```

  **Commit**: YES
  - Message: `feat: add .persistent() with block form and connection reuse`
  - Files: `ext/wreq_rb/src/lib.rs`, `lib/wreq_rb.rb`, `test/wreq_test.rb`
  - Pre-commit: `bundle exec rake ruby_test`

---

- [ ] 11. .follow() with options hash (max_hops)

  **What to do**:

  **TDD — Write tests first (RED)**:
  - Test: `HTTP.follow.get(url)` — follow redirects with default max (existing, just verify)
  - Test: `HTTP.follow(max_hops: 3).get(url)` — limited redirect following
  - Test: `HTTP.follow(max_hops: 1).get(url_with_2_redirects)` — should stop after 1 hop
  - Test: `HTTP.follow(true).get(url)` still works (backward compat)
  - Test: `HTTP.follow(false).get(url)` still works (backward compat)

  **Implement (GREEN)**:
  - Update `.follow()` to accept:
    - No args: `HTTP.follow` → follow with default (10 hops)
    - Boolean: `HTTP.follow(true/false)` → existing behavior (backward compat)
    - Hash: `HTTP.follow(max_hops: N)` → follow with N max redirects
  - Wire into `execute_request`: `Policy::limited(max_hops)` or `Policy::none()`
  - The existing implementation already uses `Policy::limited(10)` — just make the number configurable

  **Must NOT do**:
  - Do NOT implement custom redirect policies (strict, relaxed) — just max_hops

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Small enhancement to existing method, well-scoped
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 4-10)
  - **Blocks**: Task 14
  - **Blocked By**: Task 3

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/src/lib.rs:225-229` — Current `follow()` method taking bool
  - `ext/wreq_rb/src/lib.rs:247-251` — Current redirect policy application: `Policy::limited(10)` or `Policy::none()`

  **External References**:
  - http.rb: `HTTP.follow(max_hops: 3)` — from chainable.rb line 172
  - wreq redirect: `Policy::limited(n)` — same as rquest

  **Acceptance Criteria**:

  - [ ] TDD: Tests written BEFORE implementation
  - [ ] `HTTP.follow.get("https://httpbin.org/redirect/1")` follows redirect, returns 200
  - [ ] `HTTP.follow(max_hops: 1).get("https://httpbin.org/redirect/1")` returns 200 (1 hop OK)
  - [ ] `HTTP.follow(true).get(url)` still works (backward compat)
  - [ ] `HTTP.follow(false).get(url)` returns 302 (no follow, backward compat)

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: follow with max_hops
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.follow(max_hops: 5).get('https://httpbin.org/redirect/3'); exit(r.status == 200 ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: Follows up to 5 hops, 3 redirects handled
    Evidence: Script output captured

  Scenario: follow with no args (defaults)
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.follow.get('https://httpbin.org/redirect/1'); exit(r.status == 200 ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: Default follow behavior works
    Evidence: Script output captured

  Scenario: backward compat — follow(false)
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "require 'wreq-rb'; r = Wreq::HTTP.follow(false).get('https://httpbin.org/redirect/1'); exit(r.status == 302 ? 0 : 1)"
      2. Assert: exit code 0
    Expected Result: Redirects not followed when false
    Evidence: Script output captured
  ```

  **Commit**: YES
  - Message: `feat: enhance .follow() with options hash and max_hops support`
  - Files: `ext/wreq_rb/src/lib.rs`, `test/wreq_test.rb`
  - Pre-commit: `bundle exec rake ruby_test`

---

- [ ] 12. .encoding() + .flush() + remaining response methods

  **What to do**:

  **TDD — Write tests first (RED)**:
  - Test: `HTTP.encoding("UTF-8").get(url)` — forces response encoding
  - Test: `response.flush` returns self
  - Test: `response.cookies` returns Hash of response cookies (from Set-Cookie headers)

  **Implement (GREEN)**:
  - `.encoding(encoding)` — chainable method that stores desired encoding, applies to response body
  - `response.flush` — returns self (body is already eagerly read in current implementation)
  - `response.cookies` — parse Set-Cookie headers from response, return as Hash
  - These can largely be implemented in the Ruby shim layer

  **Must NOT do**:
  - Do NOT implement full cookie jar (RFC 6265 parsing) — simple key=value extraction from Set-Cookie headers
  - Do NOT change how body is stored internally

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Small additions, mostly Ruby-layer code
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Task 13)
  - **Blocks**: Task 14
  - **Blocked By**: Tasks 4, 6, 7

  **References**:

  **Pattern References**:
  - `ext/wreq_rb/src/lib.rs:539-541` — Current `headers()` method returning HashMap — pattern for cookie extraction
  - `lib/wreq_rb.rb` — Ruby shim where `.encoding()` and `.flush` could live

  **External References**:
  - http.rb `.encoding()`: Forces response body encoding
  - http.rb `.flush`: Reads/discards body for persistent connections
  - http.rb `.cookies`: Returns `HTTP::CookieJar`

  **Acceptance Criteria**:

  - [ ] TDD: Tests written BEFORE implementation
  - [ ] `HTTP.encoding("UTF-8").get(url).body.encoding.to_s` includes "UTF-8"
  - [ ] `response.flush` returns the response object itself
  - [ ] `response.cookies` returns a Hash (possibly empty)

  **Commit**: YES
  - Message: `feat: add .encoding(), response.flush, response.cookies`
  - Files: `ext/wreq_rb/src/lib.rs`, `lib/wreq_rb.rb`, `test/wreq_test.rb`
  - Pre-commit: `bundle exec rake ruby_test`

---

- [ ] 13. Update benchmarks, README, gemspec metadata, CI workflows

  **What to do**:
  - **README.md**: Complete rewrite
    - Update gem name to `wreq-rb`
    - Update all code examples to use `Wreq::HTTP`
    - Document new API methods (timeout, cookies, basic_auth, accept, persistent, via, parse, status predicates)
    - Update install instructions (`gem 'wreq-rb'`)
    - Update benchmark section
    - Mention TLS fingerprinting and http.rb drop-in replacement
    - Add migration guide section for users of old `rquest-rb`
  - **wreq-rb.gemspec**: Update description, homepage URL (if repo renamed), metadata
  - **Benchmark**: Verify `benchmark/http_clients_benchmark.rb` works with new API
  - **CI Workflows**:
    - `.github/workflows/test.yml`: Add `cmake` to apt-get install (for BoringSSL build)
    - `.github/workflows/benchmark.yml`: Update grep patterns from `Rquest-rb` → `Wreq-rb`, add cmake
    - `.github/workflows/cross-compile.yml`: Update library name from `librquest_rb` → `libwreq_rb`, add cmake
    - `.github/actions/setup-rust/action.yml`: Review for any rquest references
  - **Script files**: Update any `rquest` references in `script/*.rb`

  **Must NOT do**:
  - Do NOT change CI architecture or Ruby version matrix
  - Do NOT modify benchmark methodology

  **Recommended Agent Profile**:
  - **Category**: `writing`
    - Reason: README rewrite is documentation work, CI is config editing
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 3 (with Task 12)
  - **Blocks**: Task 14
  - **Blocked By**: Task 3 (needs final API surface)

  **References**:

  **File List** (every file to update):
  - `README.md` — full rewrite
  - `wreq-rb.gemspec` — metadata update
  - `benchmark/http_clients_benchmark.rb` — verify Wreq::HTTP usage (should already be renamed from Task 1)
  - `.github/workflows/test.yml` — add cmake install
  - `.github/workflows/benchmark.yml` — update grep patterns, add cmake
  - `.github/workflows/cross-compile.yml` — update lib name, add cmake
  - `script/visualize_benchmarks.rb` — check for rquest refs
  - `script/generate_benchmark_chart.rb` — check for rquest refs
  - `script/generate_combined_chart.rb` — check for rquest refs

  **Acceptance Criteria**:

  - [ ] `grep -r "rquest" README.md` returns NO matches (except maybe historical context)
  - [ ] `grep -r "rquest" .github/workflows/ --include="*.yml"` returns NO matches
  - [ ] README includes examples for all new API methods
  - [ ] CI workflows include `cmake` in build deps

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: README has no stale rquest references
    Tool: Bash (grep)
    Preconditions: README updated
    Steps:
      1. grep -i "rquest" README.md
      2. Assert: No functional references to old name (migration note OK)
    Expected Result: README fully reflects wreq-rb
    Evidence: grep output captured

  Scenario: CI workflow has cmake for BoringSSL
    Tool: Bash (grep)
    Preconditions: CI updated
    Steps:
      1. grep "cmake" .github/workflows/test.yml
      2. Assert: cmake present in install step
    Expected Result: Build dependency covered
    Evidence: grep output captured
  ```

  **Commit**: YES
  - Message: `docs: update README, gemspec, CI for wreq-rb with full http.rb API documentation`
  - Files: `README.md`, `wreq-rb.gemspec`, `.github/workflows/*.yml`, `script/*.rb`
  - Pre-commit: `grep -c "wreq" README.md` (should have many matches)

---

- [ ] 14. Final integration test suite + smoke test

  **What to do**:
  - **Integration tests**: Write comprehensive test file that exercises the FULL API surface end-to-end:
    - All HTTP verbs with all options combinations
    - All chainable methods composed together
    - Persistent connections with multiple requests
    - TLS fingerprinting verification (JA3/JA4 fingerprints present)
    - Error handling: timeout, invalid URL, connection refused
    - Thread safety: concurrent requests from multiple Ruby threads
    - Status predicates for various response codes
    - Response parsing (JSON, non-JSON)
    - Cookie sending and basic auth
  - **Smoke test**: Quick script that verifies the gem loads and makes a successful request
  - **Regression check**: Ensure NO test from original suite was lost or broken
  - **Run full test suite**: `bundle exec rake test` (includes Rust tests + Ruby tests)
  - **Run benchmark**: `bundle exec ruby benchmark/http_clients_benchmark.rb` to verify performance

  **Must NOT do**:
  - Do NOT add tests for features not implemented in this plan (.trace, .retriable, .nodelay)
  - Do NOT change test infrastructure (keep Minitest)

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Comprehensive integration testing, concurrency testing, verification across full API surface
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3 (final, after all others)
  - **Blocks**: None (final task)
  - **Blocked By**: ALL previous tasks (1-13)

  **References**:

  **Pattern References**:
  - `test/wreq_test.rb` — existing test file with Minitest patterns, live HTTP calls
  - `Rakefile:148-156` — `Rake::TestTask.new(:ruby_test)` — test task configuration

  **Acceptance Criteria**:

  - [ ] `bundle exec rake test` passes (exit code 0)
  - [ ] Test count >= 40 (comprehensive coverage of new API)
  - [ ] Thread safety test: 10 concurrent threads each making 5 requests, all succeed
  - [ ] Performance benchmark completes without errors
  - [ ] `ruby -e "require 'wreq-rb'; puts Wreq::HTTP::VERSION"` outputs version

  **Agent-Executed QA Scenarios:**

  ```
  Scenario: Full test suite passes
    Tool: Bash
    Preconditions: All tasks complete, extension compiled
    Steps:
      1. bundle exec rake compile
      2. bundle exec rake ruby_test
      3. Assert: exit code 0
      4. Assert: output shows 0 failures, 0 errors
      5. Assert: test count >= 40
    Expected Result: All tests green
    Evidence: Test output captured

  Scenario: Thread safety under concurrency
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "
        require 'wreq-rb'
        threads = 10.times.map do
          Thread.new do
            5.times { Wreq::HTTP.get('https://httpbin.org/get') }
            :ok
          end
        end
        results = threads.map(&:value)
        exit(results.all? { |r| r == :ok } ? 0 : 1)
        "
      2. Assert: exit code 0
    Expected Result: All threads complete without errors
    Evidence: Script output captured

  Scenario: Benchmark runs without errors
    Tool: Bash
    Preconditions: Extension compiled, benchmark deps available
    Steps:
      1. timeout 120 bundle exec ruby benchmark/http_clients_benchmark.rb
      2. Assert: exit code 0
      3. Assert: output contains "wreq-rb" benchmark results
    Expected Result: Benchmark completes with results
    Evidence: Benchmark output captured

  Scenario: Smoke test — gem loads and works
    Tool: Bash (ruby -e)
    Preconditions: Extension compiled
    Steps:
      1. ruby -e "
        require 'wreq-rb'
        puts Wreq::HTTP::VERSION
        r = Wreq::HTTP
          .headers(accept: 'application/json')
          .timeout(30)
          .follow(max_hops: 5)
          .get('https://httpbin.org/get')
        puts r.status.to_s
        puts r.status.success?
        parsed = r.parse
        puts parsed['url']
        exit 0
        "
      2. Assert: exit code 0
      3. Assert: stdout contains version, "200 OK", "true", "httpbin.org"
    Expected Result: Full API chain works end-to-end
    Evidence: Script output captured
  ```

  **Commit**: YES
  - Message: `test: add comprehensive integration tests and smoke test for wreq-rb`
  - Files: `test/wreq_test.rb`, `test/wreq_integration_test.rb`
  - Pre-commit: `bundle exec rake ruby_test`

---

## Commit Strategy

| After Task | Message | Key Files | Verification |
|------------|---------|-----------|--------------|
| 1 | `rename: rquest-rb → wreq-rb across entire project` | All renamed files | grep for old name |
| 2 | `deps: migrate from rquest 5.1.0 to wreq 6.0.0-rc.27` | Cargo.toml, lib.rs | `rake compile && rake ruby_test` |
| 3 | `refactor: DRY up HTTP methods, eliminate expect() panics` | lib.rs | `rake compile && rake ruby_test` |
| 4 | `feat: add options hash (:json, :form, :params) + .request()` | lib.rs, tests | `rake ruby_test` |
| 5 | `feat: add chainable .timeout()` | lib.rs, tests | `rake ruby_test` |
| 6 | `feat: add Status object with predicates` | lib.rs, tests | `rake ruby_test` |
| 7 | `feat: add response.parse and response.flush` | lib.rs or wreq_rb.rb, tests | `rake ruby_test` |
| 8 | `feat: add .cookies(), .basic_auth(), .auth(), .accept()` | lib.rs, tests | `rake ruby_test` |
| 9 | `feat: add .via() proxy with http.rb signature` | lib.rs, tests | `rake ruby_test` |
| 10 | `feat: add .persistent() with block form` | lib.rs, wreq_rb.rb, tests | `rake ruby_test` |
| 11 | `feat: enhance .follow() with max_hops` | lib.rs, tests | `rake ruby_test` |
| 12 | `feat: add .encoding(), response.flush, response.cookies` | lib.rs, wreq_rb.rb, tests | `rake ruby_test` |
| 13 | `docs: update README, gemspec, CI for wreq-rb` | README, gemspec, CI | grep verification |
| 14 | `test: comprehensive integration tests + smoke test` | tests | `rake test` |

---

## Success Criteria

### Verification Commands
```bash
# 1. Gem loads with new name
ruby -e "require 'wreq-rb'; puts Wreq::HTTP::VERSION"
# Expected: outputs version string (e.g., "1.0.0")

# 2. No old name references
grep -r "rquest" --include="*.rs" --include="*.rb" --include="*.toml" ext/ lib/ test/ Rakefile Cargo.toml
# Expected: no matches (exit code 1)

# 3. Full test suite passes
bundle exec rake test
# Expected: exit code 0, 0 failures, 0 errors

# 4. Compilation succeeds
bundle exec rake compile
# Expected: exit code 0

# 5. http.rb-style API works
ruby -e "
require 'wreq-rb'
r = Wreq::HTTP
  .headers(accept: 'application/json')
  .timeout(30)
  .follow(max_hops: 5)
  .get('https://httpbin.org/get')
puts r.status.success?    # true
puts r.parse.class        # Hash
puts r.status.to_s        # 200 OK
"
# Expected: true, Hash, 200 OK

# 6. TLS fingerprinting still works
ruby -e "
require 'wreq-rb'; require 'json'
r = Wreq::HTTP.get('https://tls.peet.ws/api/all')
data = JSON.parse(r.body)
puts data['tls']['ja3_hash'] ? 'TLS OK' : 'TLS FAIL'
"
# Expected: TLS OK
```

### Final Checklist
- [ ] All "Must Have" features present and tested
- [ ] All "Must NOT Have" constraints verified (no panics, no silent errors, no over-engineering)
- [ ] All tests pass (Ruby + Rust)
- [ ] Benchmark runs successfully
- [ ] CI workflows updated for wreq + BoringSSL build deps
- [ ] README documents all new API methods
- [ ] Version bumped to 1.0.0
