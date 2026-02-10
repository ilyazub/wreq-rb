# wreq-rb

A high-performance HTTP client for Ruby with TLS fingerprinting capabilities. This gem is a drop-in replacement for [http.rb](https://github.com/httprb/http) with the blazing-fast Rust [`wreq`](https://github.com/0x676e67/wreq) HTTP client powering it.

## Features

- 🚀 **Fast**: Rust-powered HTTP client with connection pooling
- 🔄 **http.rb compatible**: Drop-in replacement with familiar chainable API
- 🔐 **TLS fingerprinting**: Browser emulation (Chrome, Firefox, Safari, Edge)
- ⚡ **HTTP/2 support**: Modern protocol support out of the box
- 🧵 **Thread-safe**: Safe for high-concurrency environments
- 🎯 **Zero-copy**: Efficient Rust↔Ruby data transfer

## Installation

Add this line to your application's Gemfile:

```ruby
gem 'wreq-rb'
```

And then execute:

```
$ bundle
```

Or install it yourself as:

```
$ gem install wreq-rb
```

## Quick Start

```ruby
require 'wreq-rb'

# Simple GET request
response = HTTP.get("https://httpbin.org/get")
puts response.status  # => 200
puts response.body    # => JSON response body

# Chain configuration methods
response = HTTP
  .headers(accept: "application/json")
  .timeout(30)
  .follow(max_hops: 5)
  .get("https://httpbin.org/get")

puts response.status.success?  # => true
parsed = response.parse         # Auto-parses JSON
puts parsed["url"]              # => "https://httpbin.org/get"
```

## API Documentation

This gem is designed as a drop-in replacement for the http.rb gem with full API compatibility.

### Basic GET Request

```ruby
require 'wreq-rb'

# Simple GET request
response = HTTP.get("https://httpbin.org/get")

puts response.status  # => 200
puts response.body    # => JSON response body
```

### All HTTP Methods

```ruby
# GET, POST, PUT, DELETE, HEAD, PATCH
HTTP.get("https://httpbin.org/get")
HTTP.post("https://httpbin.org/post", body: "data")
HTTP.put("https://httpbin.org/put", body: "data")
HTTP.delete("https://httpbin.org/delete")
HTTP.head("https://httpbin.org/get")
HTTP.patch("https://httpbin.org/patch", body: "data")

# Generic request method
HTTP.request(:post, "https://httpbin.org/post", json: { foo: "bar" })
```

### Options Hash

All HTTP methods accept an options hash:

```ruby
# JSON body (auto-serialized, sets Content-Type)
HTTP.post("https://httpbin.org/post", json: { name: "Alice", age: 30 })

# Form data (URL-encoded, sets Content-Type)
HTTP.post("https://httpbin.org/post", form: { name: "Alice", email: "alice@example.com" })

# Raw body
HTTP.post("https://httpbin.org/post", body: "raw string data")

# Query parameters (appended to URL)
HTTP.get("https://httpbin.org/get", params: { q: "search", page: 2 })
```

### Chainable Configuration

Configure requests by chaining methods:

```ruby
# Multiple chainable methods
HTTP.headers(accept: "application/json")
    .timeout(30)
    .follow(max_hops: 5)
    .cookies(session: "abc123")
    .get("https://httpbin.org/get")

# Timeout
HTTP.timeout(30).get("https://httpbin.org/delay/5")

# Follow redirects (default: 10 max hops)
HTTP.follow.get("https://httpbin.org/redirect/3")
HTTP.follow(max_hops: 5).get("https://httpbin.org/redirect/3")
HTTP.follow(false).get("https://httpbin.org/redirect/1")  # => 302

# Authentication
HTTP.basic_auth(user: "username", pass: "password")
    .get("https://httpbin.org/basic-auth/username/password")
HTTP.auth("Bearer token").get("https://api.example.com/protected")

# Accept header shortcuts
HTTP.accept(:json).get("https://httpbin.org/get")  # => Accept: application/json
HTTP.accept(:xml).get("https://httpbin.org/xml")    # => Accept: application/xml

# Proxy (http.rb-style)
HTTP.via("proxy.example.com", 8080).get("https://httpbin.org/ip")
HTTP.via("proxy.example.com", 8080, "user", "pass").get("https://httpbin.org/ip")

# Encoding
HTTP.encoding("UTF-8").get("https://httpbin.org/get")
```

### Response Object

Rich response object with status predicates and auto-parsing:

```ruby
response = HTTP.get("https://httpbin.org/get")

# Status object with predicates
response.status.success?        # => true (2xx)
response.status.ok?              # => true (exactly 200)
response.status.redirect?        # => false (3xx)
response.status.client_error?    # => false (4xx)
response.status.server_error?    # => false (5xx)
response.status.to_s            # => "200 OK"
response.status.reason          # => "OK"

# Auto-parse JSON responses
parsed = response.parse  # => Hash (if Content-Type: application/json)

# Response data
response.body         # => String
response.headers      # => Hash
response.content_type # => "application/json"
response.cookies      # => Hash (parsed from Set-Cookie)
response.code         # => 200 (integer, backward compat)
```

### TLS Fingerprinting

Emulate browser TLS fingerprints to bypass bot detection:

```ruby
# Random desktop browser (Chrome, Firefox, Safari, Edge)
HTTP.desktop.get("https://tls.peet.ws/api/all")

# Random mobile browser
HTTP.mobile.get("https://tls.peet.ws/api/all")

# Chain with other methods
HTTP.desktop
    .headers(accept: "application/json")
    .timeout(30)
    .get("https://api.example.com")
```

### Complete Example

```ruby
require 'wreq-rb'

# Complex request with multiple features
response = HTTP
  .headers(accept: "application/json", x_api_key: "secret")
  .timeout(30)
  .follow(max_hops: 3)
  .cookies(session: "abc123")
  .basic_auth(user: "api", pass: "password")
  .post(
    "https://api.example.com/data",
    json: {
      query: "search term",
      filters: { category: "books", limit: 10 }
    }
  )

if response.status.success?
  data = response.parse
  puts "Found #{data['results'].length} results"
else
  puts "Error: #{response.status}"
end
```

## Benchmarks

wreq-rb is designed to be a high-performance alternative to other Ruby HTTP clients. Here's how it compares:

### Running Benchmarks

The project includes benchmarks to compare wreq-rb with other popular Ruby HTTP clients.

```
$ bundle exec ruby benchmark/http_clients_benchmark.rb
```

This will run a benchmark making 5,000 requests to a test endpoint with concurrency, comparing multiple HTTP clients.

Recent benchmark results:
```
Comparison:
                curb:       59.7 i/s
            typhoeus:       47.6 i/s - 1.25x  slower
             wreq-rb:       19.5 i/s - 3.07x  slower
             http.rb:       10.6 i/s - 5.61x  slower
               httpx:        9.0 i/s - 6.64x  slower
```

As shown above, curb is the fastest client, with typhoeus following closely. Wreq-rb provides excellent performance, significantly outperforming both http.rb and httpx in sequential operations.

### Benchmark History

Benchmarks are automatically run on every push to the master branch using GitHub Actions. This allows us to track performance over time and ensure wreq-rb maintains its performance advantage.

#### Performance Trend Visualization

Benchmark charts are generated for multiple Ruby versions (2.7, 3.0, 3.1, 3.2, 3.3) to track performance across different Ruby implementations.

##### Combined Performance Comparison
The following chart shows how wreq-rb compares to other HTTP clients across all tested Ruby versions:

![Combined HTTP Client Performance](https://github.com/0x676e67/wreq-rb/raw/main/docs/assets/combined_time_chart.png)

As shown in our latest benchmarks, curb is the fastest client, with typhoeus being a close second. Wreq-rb provides excellent performance, significantly outperforming both HTTP.rb and httpx across all Ruby versions.

##### Ruby 2.7 (default)
![Request Time Benchmark Chart (Ruby 2.7)](https://github.com/0x676e67/wreq-rb/raw/main/docs/assets/time_chart-2.7.png)
![Requests Per Second Benchmark Chart (Ruby 2.7)](https://github.com/0x676e67/wreq-rb/raw/main/docs/assets/rps_chart-2.7.png)

For performance charts of other Ruby versions, see the [benchmark summary page](https://github.com/0x676e67/wreq-rb/blob/main/docs/assets/benchmark_summary.md).

*Note: These charts are automatically generated during CI runs. The latest charts can be found in the GitHub Actions artifacts.*

#### Raw Benchmark Data

You can find historical benchmark results in the GitHub Actions artifacts. Each run stores:
- A detailed benchmark result for each Ruby version
- CSV files with historical benchmark data for each Ruby version
- Graphviz charts in PNG and SVG formats

To visualize benchmark history, download the `benchmark-history-{ruby_version}.csv` artifact and use the provided script:

```
$ script/visualize_benchmarks.rb -f benchmark-history-2.7.csv
```

Options:
- `-f, --file FILE` - Specify the CSV file path
- `-m, --metric TYPE` - Metric to visualize (time or requests_per_second)
- `-l, --limit NUM` - Limit to last N entries

## Development

After checking out the repo, install dependencies and build the extension:

```
$ bundle install
$ bundle exec rake compile
```

To run tests:

```
$ bundle exec rake test
```

### Build Requirements

wreq-rb uses BoringSSL for TLS, which requires:
- `cmake` - Build system
- `perl` - For BoringSSL build scripts  
- `libclang-dev` - For bindgen (Rust FFI generation)

On Ubuntu/Debian:
```bash
sudo apt-get install -y cmake perl libclang-dev
```

On macOS:
```bash
brew install cmake
```

## Migration from rquest-rb

This gem was renamed from `rquest-rb` to `wreq-rb` following the upstream Rust crate rename. The API remains fully compatible. To migrate:

1. Update your Gemfile: `gem 'rquest-rb'` → `gem 'wreq-rb'`
2. Update requires: `require 'rquest-rb'` → `require 'wreq-rb'`
3. Module references work via top-level `HTTP` constant (no changes needed)

## Contributing

1. Fork it
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -am 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Create a new Pull Request

## License

The gem is available as open source under the terms of the [MIT License](https://opensource.org/licenses/MIT).
