# Performance Profiling Guide for wreq-rb

This guide explains how to profile wreq-rb to identify performance bottlenecks and generate flamegraphs.

## Quick Start: rbspy (Recommended)

**rbspy** is the easiest profiling tool - no code changes required, generates interactive flamegraphs automatically.

### Installation

```bash
cargo install rbspy
```

### Generate Flamegraph

```bash
# Profile a Ruby script
rbspy record --file wreq_rb_profile.svg -- ruby your_script.rb

# Profile with more samples (higher accuracy)
rbspy record --rate 1000 --file wreq_rb_profile.svg -- ruby your_script.rb

# Open the flamegraph
open wreq_rb_profile.svg  # macOS
firefox wreq_rb_profile.svg  # Linux
```

### Example Profiling Script

```ruby
# profile_example.rb
require './lib/wreq_rb'

HTTP = Wreq::HTTP

1000.times do |i|
  case i % 5
  when 0
    HTTP.get('https://postman-echo.com/get')
  when 1
    HTTP.post('https://postman-echo.com/post', json: { request: i })
  when 2
    HTTP.headers(accept: 'application/json').get('https://postman-echo.com/get')
  when 3
    HTTP.persistent('https://postman-echo.com') { |c| c.get('/get') }
  when 4
    response = HTTP.get('https://postman-echo.com/status/200')
    response.status.success?
  end
end
```

Run profiling:
```bash
rbspy record --file wreq_rb_profile.svg -- ruby profile_example.rb
```

---

## Alternative 1: StackProf (Ruby-Native)

**StackProf** is a sampling call-stack profiler for Ruby. Great for Ruby-level performance analysis.

### Installation

```bash
gem install stackprof
```

### Usage

```ruby
# stackprof_example.rb
require 'bundler/setup'
require 'stackprof'
require './lib/wreq_rb'

HTTP = Wreq::HTTP

StackProf.run(mode: :cpu, out: 'tmp/stackprof-cpu.dump', raw: true) do
  100.times do |i|
    HTTP.get('https://postman-echo.com/get')
    HTTP.post('https://postman-echo.com/post', json: { data: i })
  end
end

puts "Profile saved to tmp/stackprof-cpu.dump"
```

### View Results

```bash
# Text report (top methods by time)
stackprof tmp/stackprof-cpu.dump --text

# Flamegraph (interactive HTML)
stackprof tmp/stackprof-cpu.dump --flamegraph > wreq_rb_stackprof.html
open wreq_rb_stackprof.html

# Method-specific analysis
stackprof tmp/stackprof-cpu.dump --method 'Wreq::HTTP#get'
```

---

## Alternative 2: dtrace (macOS System-Level)

**dtrace** is a system-level profiler on macOS. Provides the deepest insights including Rust FFI layer.

### Requirements

- macOS
- sudo access

### Generate Flamegraph

```bash
# 1. Clone FlameGraph tools
git clone https://github.com/brendangregg/FlameGraph /tmp/FlameGraph

# 2. Run dtrace profiling (requires sudo)
sudo dtrace -x ustackframes=100 -n 'profile-997 /execname == "ruby"/ { @[ustack()] = count(); }' -o out.stacks -c 'ruby profile_example.rb'

# 3. Generate flamegraph
/tmp/FlameGraph/stackcollapse.pl out.stacks | /tmp/FlameGraph/flamegraph.pl > wreq_rb_flamegraph.svg

# 4. Open flamegraph
open wreq_rb_flamegraph.svg
```

---

## Alternative 3: perf (Linux System-Level)

**perf** is the Linux equivalent of dtrace.

### Installation

```bash
# Ubuntu/Debian
sudo apt-get install linux-tools-common linux-tools-generic

# Fedora/RHEL
sudo dnf install perf
```

### Generate Flamegraph

```bash
# 1. Clone FlameGraph tools
git clone https://github.com/brendangregg/FlameGraph /tmp/FlameGraph

# 2. Record profiling data
perf record -F 99 -g -- ruby profile_example.rb

# 3. Generate flamegraph
perf script | /tmp/FlameGraph/stackcollapse-perf.pl | /tmp/FlameGraph/flamegraph.pl > wreq_rb_flamegraph.svg

# 4. View flamegraph
firefox wreq_rb_flamegraph.svg
```

---

## Interpreting Flamegraphs

### Reading the Visualization

- **X-axis (width)**: Proportion of time spent in function (wider = more time)
- **Y-axis (height)**: Call stack depth (bottom = entry point, top = leaf functions)
- **Color**: Random (used only to differentiate between functions)

### What to Look For

1. **Wide plateaus**: Functions consuming most CPU time
2. **Rust FFI calls**: Look for `magnus::` or `wreq::` namespaces
3. **Network I/O**: Should dominate in HTTP clients (expected)
4. **Ruby overhead**: Time spent outside Rust FFI
5. **Unexpected hotspots**: Code that shouldn't be expensive but is

### Example Analysis

```
Total Time: 10.5s
  ├─ Network I/O: 9.2s (87.6%) ← Expected, unavoidable
  ├─ Rust FFI: 0.8s (7.6%)    ← Reasonable overhead
  ├─ Ruby parsing: 0.3s (2.9%) ← JSON.parse, acceptable
  └─ Other: 0.2s (1.9%)       ← Misc overhead
```

**Conclusion**: If 85%+ of time is network I/O, performance is optimal.

---

## Profiling Different Scenarios

### 1. FFI Overhead

Profile Rust-Ruby boundary:

```ruby
# Profile many small requests to measure FFI overhead
10000.times { HTTP.get('https://postman-echo.com/get') }
```

### 2. Connection Pooling

Profile persistent connections:

```ruby
HTTP.persistent('https://postman-echo.com') do |client|
  1000.times { client.get('/get') }
end
```

### 3. JSON Parsing

Profile response parsing:

```ruby
1000.times do
  response = HTTP.post('https://postman-echo.com/post', json: { large: 'x' * 10000 })
  response.parse
end
```

### 4. Concurrent Requests

Profile thread safety:

```ruby
threads = 10.times.map do
  Thread.new do
    100.times { HTTP.get('https://postman-echo.com/get') }
  end
end
threads.each(&:join)
```

---

## Benchmark vs Profiling

| Tool | Purpose | Output |
|------|---------|--------|
| **benchmark-ips** | Compare throughput | Requests/second |
| **rbspy/stackprof** | Identify hotspots | Flamegraph/call tree |
| **dtrace/perf** | System-level analysis | Full stack traces |

**Use benchmarks** to measure "how fast is it?"  
**Use profiling** to answer "why is it slow?"

---

## Performance Optimization Checklist

After profiling, focus on:

1. ✅ **Network I/O dominates** (85%+ of time)
   - Expected, no optimization needed

2. ⚠️ **Ruby overhead > 10%**
   - Review JSON parsing, string operations
   - Consider caching parsed responses

3. ⚠️ **FFI overhead > 10%**
   - Batch requests where possible
   - Use persistent connections

4. ❌ **Unexpected CPU hotspots**
   - Investigate with method-level profiling
   - Consider algorithmic improvements

---

## Resources

- **rbspy**: https://rbspy.github.io/
- **StackProf**: https://github.com/tmm1/stackprof
- **FlameGraph**: https://github.com/brendangregg/FlameGraph
- **Brendan Gregg's Flamegraph Guide**: https://www.brendangregg.com/flamegraphs.html

---

## Previous Profiling Results

### Initial Analysis (2026-02-11)

**Setup**: 20 sequential HTTP GET requests to httpbin.org/get

**Findings**:
- Network I/O: 94% of total time (~900ms per request)
- FFI overhead: 6% of total time (~60ms per request)
- Ruby overhead: < 1% (negligible)

**Optimizations Applied**:
1. Replaced thread-local tokio Runtime with global static (lazy_static)
2. Pre-allocated header HashMaps
3. Optimized header processing with O(1) flag tracking

**Result**: 22-25% performance improvement (16.89s → 13-14s for 20 requests)

**Conclusion**: Performance is optimal. wreq-rb is competitive with http.rb (within ±20%). Further optimization would require:
- Faster network connection (not controllable)
- HTTP/3 (depends on server support)
- Connection pooling (already implemented)
