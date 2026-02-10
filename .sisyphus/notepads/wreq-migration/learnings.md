# Learnings - wreq-migration

## Session: ses_3b9192b8affel60iynWw4Ub243
Started: 2026-02-10T12:19:50.496Z

---

## [2026-02-10 12:40] Task 5: Chainable .timeout()

**Status**: ✅ COMPLETE (Commit dd938c6)

**Implementation Pattern**:
```rust
// Instance method (line 341-346)
fn timeout(&self, secs: u64) -> Self {
    let mut new_client = self.clone();
    new_client.timeout = secs;
    new_client
}

// Module-level wrapper (line 610-612)
fn rb_timeout(secs: u64) -> Result<RbHttpClient, MagnusError> {
    Ok(RbHttpClient::new()?.timeout(secs))
}

// Ruby registration (lines 639, 659)
client_class.define_method("timeout", method!(RbHttpClient::timeout, 1))?;
http_module.define_module_function("timeout", function!(rb_timeout, 1))?;
```

**Ruby API**:
```ruby
HTTP.timeout(30).get("https://httpbin.org/get")
HTTP.headers(accept: "application/json").timeout(10).get(url)
```

**Key Insights**:
- Chainable methods follow consistent pattern: clone → mutate → return
- Module-level functions wrap `new()? + instance_method()` pattern
- Tests verify both standalone and chained usage
- All wreq imports verified intact (no corruption)

**Files Modified**:
- `ext/wreq_rb/src/lib.rs`: Added timeout() method + rb_timeout() function
- `test/wreq_test.rb`: Added 2 tests (chainable, with_headers)


## [2026-02-10 13:10] Task 6: Response Status Object with Predicates

**Status**: ✅ COMPLETE (Commit 5f4b56b)

**Implementation Pattern**:
```rust
// Status struct with magnus::wrap (line 19-132)
#[magnus::wrap(class = "Wreq::HTTP::Status", free_immediately, size)]
struct RbHttpStatus {
    code: u16,
}

impl RbHttpStatus {
    fn new(code: u16) -> Self { Self { code } }
    fn to_i(&self) -> u16 { self.code }
    fn to_s(&self) -> String { format!("{} {}", self.code, self.reason()) }
    fn reason(&self) -> &'static str { /* 91 status codes */ }
    fn success(&self) -> bool { (200..300).contains(&self.code) }
    fn ok(&self) -> bool { self.code == 200 }
    fn redirect(&self) -> bool { (300..400).contains(&self.code) }
    fn client_error(&self) -> bool { (400..500).contains(&self.code) }
    fn server_error(&self) -> bool { (500..600).contains(&self.code) }
    fn informational(&self) -> bool { (100..200).contains(&self.code) }
    fn eq(&self, other: Value) -> Result<bool, MagnusError> { /* equality */ }
}

// Update response.status() to return Status object (line ~757)
fn status(&self) -> RbHttpStatus {
    RbHttpStatus::new(self.data.status)
}

// Keep response.code() for backward compat (line ~784)
fn code(&self) -> u16 {
    self.data.status  // Direct field access, not status() call
}

// Ruby registration (lines ~760-775)
let status_class = http_module.define_class("Status", ruby.class_object())?;
status_class.define_method("to_i", method!(RbHttpStatus::to_i, 0))?;
status_class.define_method("success?", method!(RbHttpStatus::success, 0))?;
status_class.define_method("==", method!(RbHttpStatus::eq, 1))?;
```

**Ruby API**:
```ruby
response.status.success?         # true for 2xx
response.status.ok?               # true for 200
response.status.redirect?         # true for 3xx
response.status.client_error?     # true for 4xx
response.status.server_error?     # true for 5xx
response.status.reason            # "OK", "Not Found", etc
response.status.to_s              # "200 OK"
response.status.to_i              # 200
response.status == 200            # true (equality)
response.code                     # 200 (backward compat)
```

**Key Insights**:
- Status object wraps u16 code, provides predicates + reason phrases
- 91 HTTP status codes from RFC 9110 implemented
- Equality operator supports both Status-to-Status and Status-to-Integer comparison
- `response.code` kept for backward compatibility (returns u16 directly)
- `response.status` now returns Status object (breaking change but http.rb compatible)
- Rust tests updated: `.status()` → `.status().to_i()` for assertions
- All wreq imports verified intact (no corruption)

**Files Modified**:
- `ext/wreq_rb/src/lib.rs`: Added Status struct (133 lines), updated response methods, fixed tests
- `test/wreq_test.rb`: Added 5 tests (status predicates, equality, 404, redirect, backward compat)

**Backward Compatibility**:
- `.code` still returns Integer (unchanged)
- `.status` now returns Status object (was Integer) - breaking but matches http.rb

