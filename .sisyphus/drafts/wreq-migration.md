# Draft: Migrate wreq-rb to wreq upstream

## Requirements (confirmed)
- Update from `rquest` (v5.1.0) + `rquest-util` (v2.1.0) to `wreq` (latest) + `wreq-util` (latest)
- Keep Ruby API "as simple as possible, as Ruby way as possible"
- Target "100% success rate and blazing fast and safe"
- Upstream repo renamed: `0x676e67/rquest` → `0x676e67/wreq`

## Technical Decisions
- **Upstream crate**: `wreq` 6.0.0-rc.27, `wreq-util` 3.0.0-rc.9 (all versions are RC currently)
- **Import path changes**: `rquest::*` → `wreq::*`, `rquest_util::*` → `wreq_util::*`
- **BoringSSL**: wreq uses BoringSSL, needs cmake/perl/libclang-dev for build
- **Compression**: wreq uses tower-http for compression (feature flags changed)
- **Emulation variants**: Updated names (e.g., Safari26 instead of Safari17_0)
- **License change**: Apache-2.0 (wreq) vs GPL-3.0 (old rquest)

## Research Findings
- **Project structure**: Ruby gem wrapping Rust via magnus + rb_sys native extension
- **Core file**: `ext/wreq_rb/src/lib.rs` (787 lines) — all HTTP logic lives here
- **Ruby shim**: `lib/wreq_rb.rb` — just loads the native extension
- **Tests**: `test/wreq_test.rb` (303 lines, Minitest, live HTTP calls)
- **Anti-patterns found** (from explore agent):
  - `expect()` panics on client/runtime creation (should map to Ruby exceptions)
  - Massive code duplication across HTTP methods (get/post/put/delete/head/patch)
  - Silent error swallowing on body read (Err → None)
  - `clone()` rebuilds entire TLS client (expensive)
  - Tokio runtime conflicts (tests skip due to Ruby thread context issues)
  - Generic `runtime_error` instead of typed error classes

## Decisions Made (from user interview)
1. **Rename**: YES — full rename from `rquest-rb` to `wreq-rb`, `Rquest::HTTP` → `Wreq::HTTP`
2. **Version pinning**: Use latest RC — `wreq 6.0.0-rc.27`, `wreq-util 3.0.0-rc.9`
3. **Ruby API**: More idiomatic Ruby, drop-in replacement for http.rb gem (used in production for years)
4. **Refactoring**: Full cleanup — DRY up methods, proper error handling, fix clone(), typed errors

## http.rb Drop-in Replacement Goal
User explicitly stated: "More idiomatic Ruby and idiomatic http.rb. The idea to be a drop-in replacement to http.rb which we use in production for years."

### Current API vs http.rb Gaps (to investigate):
- http.rb uses chainable API: `HTTP.headers(...).timeout(...).follow.get(url)`
- http.rb has `.body()`, `.form()`, `.json()` for request body types
- http.rb has `HTTP::Response` with `.status` (object with .ok?, .success?), `.parse` (auto JSON)
- http.rb has `HTTP.timeout(connect: N, read: N, write: N)`
- http.rb has `HTTP.cookies(...)`, `HTTP.basic_auth(...)`, `HTTP.accept(:json)`
- http.rb has `HTTP.persistent("https://...")` for connection reuse

### Current wreq-rb API (from code analysis):
- Module methods: `HTTP.get`, `.post`, `.put`, `.delete`, `.head`, `.patch`, `.headers`, `.follow`, `.proxy`, `.desktop`, `.mobile`
- Client methods: same HTTP verbs + `.with_headers`, `.with_proxy`, `.follow`, `.headers`
- Response: `.status` (u16), `.body` (String), `.headers` (Hash), `.content_type`, `.uri`, `.code`, `.charset`, `.to_s`
- Missing vs http.rb: `.timeout()`, `.cookies()`, `.basic_auth()`, `.accept()`, `.json()`, `.form()`, `.parse`, status predicates

## Scope Boundaries
- INCLUDE: Full gem rename, Cargo dep update, Rust source migration, Ruby shim updates, API improvements toward http.rb compat, full code cleanup, tests, CI
- EXCLUDE: TBD — need to determine how close to http.rb compat we go in v1

## Decisions Made (additional)
3. **http.rb parity**: FULL parity — including persistent connections (used in production)
4. **Test strategy**: TDD, GOOSGBT (Growing Object-Oriented Software Guided By Tests)
5. **No premature abstractions or optimizations** — user's explicit constraint
6. **High concurrency** — will run in highly concurrent environment
7. **AI-verifiable tests** — tests must be digestible by AI and by human team

## http.rb Full Chainable API (from source: lib/http/chainable.rb)

### HTTP Verbs (all take `uri, options = {}`)
- `.get(uri, options)` ✅ HAVE (but different signature)
- `.post(uri, options)` ✅ HAVE (but different signature)
- `.put(uri, options)` ✅ HAVE (but different signature)
- `.delete(uri, options)` ✅ HAVE (but different signature)
- `.head(uri, options)` ✅ HAVE (but different signature)
- `.patch(uri, options)` ✅ HAVE (but different signature)
- `.trace(uri, options)` ❌ MISSING (low priority)
- `.options(uri, options)` ❌ MISSING (low priority)
- `.connect(uri, options)` ❌ MISSING (low priority)
- `.request(verb, uri, options)` ❌ MISSING — generic request method

### Options hash keys (passed to HTTP verbs):
- `:body` — raw body string ✅ HAVE (via extract_body)
- `:json` — auto-serialize to JSON ❌ MISSING
- `:form` — URL-encoded form data ❌ MISSING
- `:params` — query string parameters ❌ MISSING

### Chainable Config Methods
- `.headers(hash)` ✅ HAVE
- `.follow(options = {})` ✅ HAVE (but takes bool, not options hash)
- `.timeout(options)` ❌ MISSING — accepts Numeric (global) or Hash (connect/read/write)
- `.cookies(hash)` ❌ MISSING
- `.accept(type)` ❌ MISSING — shortcut for Accept header with MIME normalization
- `.auth(value)` ❌ MISSING — sets Authorization header
- `.basic_auth(user:, pass:)` ❌ MISSING — Base64 auth header
- `.via(*proxy)` / `.through` ⚠️ PARTIAL — we have `.proxy(url_string)`, http.rb uses `.via(host, port, user, pass)`
- `.persistent(host, timeout: 5)` ❌ MISSING — with block form for auto-close
- `.encoding(encoding)` ❌ MISSING
- `.nodelay` ❌ MISSING — TCP_NODELAY
- `.use(*features)` ❌ MISSING — feature toggles (auto_inflate, logging, etc.)
- `.retriable(**options)` ❌ MISSING — retry with backoff

### Response Object (HTTP::Response)
- `.status` — returns HTTP::Response::Status (not just integer!) ⚠️ PARTIAL — we return u16
  - `.status.success?` (2xx) ❌ MISSING
  - `.status.redirect?` (3xx) ❌ MISSING
  - `.status.client_error?` (4xx) ❌ MISSING
  - `.status.server_error?` (5xx) ❌ MISSING
  - `.status.informational?` (1xx) ❌ MISSING
  - `.status.reason` → "OK", "Not Found" ❌ MISSING
  - `.status.to_s` → "200 OK" ❌ MISSING
  - `.status == 200` (coercion to int for comparison) ❌ MISSING
- `.code` → Integer ✅ HAVE
- `.body` → HTTP::Response::Body (not String!) ⚠️ PARTIAL — we return String
  - `.body.to_s` → String ❌ N/A (our .body already returns String)
  - `.body.each { |chunk| }` — streaming ❌ MISSING
  - `.body.readpartial` — chunked reading ❌ MISSING
- `.headers` → HTTP::Headers ⚠️ PARTIAL — we return HashMap
- `.content_type` ✅ HAVE
- `.cookies` → CookieJar ❌ MISSING
- `.parse` → auto-parse body based on MIME type ❌ MISSING
- `.to_s` ✅ HAVE
- `.uri` ✅ HAVE
- `.flush` ❌ MISSING — discard body for persistent connections
- `.charset` ✅ HAVE

### wreq-rb Unique Features (NOT in http.rb — keep these!)
- `.desktop` — desktop browser emulation
- `.mobile` — mobile browser emulation
- TLS fingerprinting / browser emulation
- Automatic random user-agent rotation

## Priority for Full Parity (ordered by user impact)

### P0 — Must have for drop-in replacement:
1. Options hash with `:body`, `:json`, `:form`, `:params`
2. `.timeout(connect:, read:, write:)` and `.timeout(seconds)`
3. `.follow(options)` — with max_hops support
4. Status object with predicate methods (`.success?`, `.ok?`, etc.)
5. `.cookies(hash)` chainable
6. `.persistent(host, timeout:)` with block form
7. `.basic_auth(user:, pass:)` and `.auth(value)`
8. `.accept(type)` with MIME normalization
9. `.via(host, port, user, pass)` proxy (http.rb signature)
10. Response `.parse` — auto JSON/form parsing
11. `.request(verb, uri, options)` — generic method

### P1 — Nice to have:
1. Response body streaming (`.body.each`, `.readpartial`)
2. `.encoding(encoding)` 
3. `.nodelay`
4. `.use(*features)` — feature system
5. `.retriable(**options)` — retry support
6. `.trace`, `.options`, `.connect` verbs

## Open Questions (remaining)
None — all requirements clear. Ready for plan generation.
