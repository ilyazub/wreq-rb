# Remove Obsolete Code from Rakefile and README

## TL;DR

> **Quick Summary**: Clean Rakefile (remove dead memcheck/benchmark tasks) and README (remove stale benchmark section with broken image links). Files already deleted via bash; these are the remaining edits.
> 
> **Deliverables**:
> - Clean Rakefile with only working tasks
> - Clean README without dead benchmark history section
> - Atomic commit of all changes
> 
> **Estimated Effort**: Quick
> **Parallel Execution**: NO - sequential
> **Critical Path**: Task 1 → Task 2 → Task 3

---

## Context

### Original Request
User asked to remove all obsolete things from docs, .sisyphus, benchmark, script, readme, code, and rake.

### Already Completed (by Prometheus via bash)
- ✅ Deleted `.sisyphus/` (17 planning artifact files)
- ✅ Deleted `docs/PROFILING.md` and `docs/NETWORK_PROFILING.md` (and empty `docs/` dir)
- ✅ Deleted `benchmark/rust_profiling_benchmark.rs`, `benchmark/http_clients_benchmark.rs`, `benchmark/http_clients_benchmark.sh`, `benchmark/benchmark_results.md`
- ✅ Deleted `script/` directory (3 graphviz chart generator scripts)
- ✅ Deleted `.github/workflows/benchmark.yml`

### Remaining Work (requires file editing)
- Rakefile: remove dead tasks referencing deleted files
- README.md: remove stale benchmark section referencing deleted scripts/images

---

## Work Objectives

### Core Objective
Remove dead code references from Rakefile and README that point to files we just deleted.

### Must NOT Have (Guardrails)
- Do NOT remove working Rakefile tasks (compile, test, cross_compile, gem tasks, fmt, rust_test, ruby_test)
- Do NOT remove the `benchmark:http_clients_rb` task — `benchmark/http_clients_benchmark.rb` still exists and works
- Do NOT touch the README API documentation sections — only the Benchmark History subsection with dead image links
- Do NOT add new content — only remove obsolete content

---

## TODOs

- [x] 1. Clean Rakefile — remove dead tasks and references

  **What to do**:
  Remove these specific blocks from `Rakefile`:

  1. Remove `require "ruby_memcheck"` (line 96) — gem may not be installed, references non-existent `test/memory_leak_test.rb`

  2. Remove the entire `namespace :test do ... end` block (lines 98-134) — both `memcheck` and `memcheck_quick` tasks reference non-existent `test/memory_leak_test.rb`

  3. Remove `task memcheck: "test:memcheck"` (line 137)

  4. Remove the `benchmark:http_clients_sh` task (lines 167-171) — references deleted `benchmark/http_clients_benchmark.sh`

  5. Update the aggregate benchmark task (line 175) to only use the Ruby benchmark:
     - Change `task :benchmark => ['benchmark:http_clients_rb', 'benchmark:http_clients_sh']`
     - To: `task :benchmark => ['benchmark:http_clients_rb']`

  6. Remove `benchmark` from the default task (line 177) since benchmarks shouldn't run by default:
     - Change `task default: %i[compile test benchmark] do`
     - To: `task default: %i[compile test] do`

  **Must NOT do**:
  - Do NOT remove `benchmark:http_clients_rb` task — it references `benchmark/http_clients_benchmark.rb` which still exists
  - Do NOT remove compile, test, cross_compile, gem, fmt, rust_test, ruby_test tasks

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: [`git-master`]

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocks**: Task 2, Task 3

  **References**:
  - `Rakefile:96` — `require "ruby_memcheck"` to remove
  - `Rakefile:98-134` — `namespace :test` block with memcheck tasks to remove
  - `Rakefile:137` — `task memcheck:` alias to remove
  - `Rakefile:167-171` — `benchmark:http_clients_sh` task to remove
  - `Rakefile:175` — aggregate benchmark task to simplify
  - `Rakefile:177` — default task to simplify

  **Acceptance Criteria**:
  - [ ] `ruby -c Rakefile` → syntax OK
  - [ ] `bundle exec rake -T` → lists compile, test, ruby_test, rust_test, fmt, benchmark:http_clients_rb (no memcheck, no benchmark:http_clients_sh)
  - [ ] `bundle exec rake compile` → succeeds
  - [ ] No references to `memory_leak_test.rb`, `ruby_memcheck`, or `http_clients_benchmark.sh` remain in Rakefile

  **Commit**: NO (group with Task 2)

- [x] 2. Clean README — remove stale benchmark section

  **What to do**:
  In `README.md`, remove the stale **Benchmark History** subsection and everything below it until the **Development** section. Specifically:

  1. Keep the "Running Benchmarks" subsection (lines 219-226) and the command: `bundle exec ruby benchmark/http_clients_benchmark.rb`

  2. Keep the "Recent benchmark results" block (lines 229-237) — these are reasonable representative numbers

  3. Keep the summary line (line 239): "As shown above, curb is the fastest..."

  4. **REMOVE everything from "### Benchmark History" (line 241) through the end of the `script/visualize_benchmarks.rb` instructions (line 280)**. This includes:
     - "Benchmark History" header and description (lines 241-243)
     - "Performance Trend Visualization" subsection (lines 245-262) — references non-existent `docs/assets/` images
     - "Raw Benchmark Data" subsection (lines 264-280) — references deleted `script/visualize_benchmarks.rb`

  5. The "## Development" section (line 282) should immediately follow the benchmark summary.

  **Must NOT do**:
  - Do NOT remove the Quick Start, API Documentation, TLS Fingerprinting, or any other sections
  - Do NOT modify the "Running Benchmarks" or "Recent benchmark results" parts
  - Do NOT remove the Migration from rquest-rb section

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: [`git-master`]

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Blocked By**: Task 1
  - **Blocks**: Task 3

  **References**:
  - `README.md:241-280` — the stale benchmark history section to remove
  - `README.md:282` — the Development section that should follow benchmarks

  **Acceptance Criteria**:
  - [ ] README has "## Benchmarks" section with running instructions and recent results
  - [ ] No references to `docs/assets/`, `script/visualize_benchmarks.rb`, `benchmark-history-*.csv`, or chart images
  - [ ] "## Development" immediately follows the benchmark comparison text
  - [ ] No broken markdown links remain

  **Commit**: NO (group with Task 3)

- [ ] 3. Commit all cleanup changes atomically

  **What to do**:
  Stage ALL changes (deletions + edits) and commit:
  
  ```bash
  git add -A
  git commit -m "remove obsolete docs, benchmarks, scripts, and dead code"
  ```

  **Must NOT do**:
  - Do NOT create multiple commits — one atomic commit for the entire cleanup
  - Do NOT push

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: [`git-master`]

  **Parallelization**:
  - **Blocked By**: Task 1, Task 2

  **Acceptance Criteria**:
  - [ ] `git status` → clean working tree
  - [ ] `git log -1 --stat` → shows all deleted files + Rakefile + README edits in one commit
  - [ ] `bundle exec rake compile` → succeeds
  - [ ] `bundle exec ruby test/wreq_test.rb` → 77 tests, 0 failures, 0 errors

  **Commit**: YES
  - Message: `remove obsolete docs, benchmarks, scripts, and dead code`
  - Files: all staged deletions + Rakefile + README.md

---

## Success Criteria

### Verification Commands
```bash
git status                           # clean working tree
bundle exec rake -T                  # no memcheck, no benchmark:http_clients_sh
bundle exec rake compile             # still builds
bundle exec ruby test/wreq_test.rb   # 77 tests, 0 failures
```

### Final Checklist
- [ ] No references to deleted files remain in Rakefile or README
- [ ] Working tasks preserved (compile, test, benchmark:http_clients_rb)
- [ ] Single atomic commit
