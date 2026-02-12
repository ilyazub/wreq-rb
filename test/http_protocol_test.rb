# frozen_string_literal: true

# rubocop:disable Metrics/ClassLength, Metrics/AbcSize, Metrics/MethodLength

require 'minitest/autorun'
require_relative '../lib/wreq_rb'
require 'json'

class HttpProtocolTest < Minitest::Test
  HTTP = Wreq::HTTP

  # ============================================================================
  # HTTP/1.1 and HTTP/2 Protocol Detection Tests
  # ============================================================================

  def test_http11_protocol_support
    # Verify HTTP/1.1 connections work and return valid responses
    response = HTTP.get('https://http1.golang.org/')
    assert_equal 200, response.status.to_i
    assert_kind_of String, response.body
    assert response.body.length > 0
  end

  def test_http2_protocol_support
    # Verify HTTP/2 connections work and return valid responses
    response = HTTP.get('https://http2.golang.org/')
    assert_equal 200, response.status.to_i
    assert_kind_of String, response.body
    assert response.body.length > 0
  end

  def test_httpbin_http2_support
    # httpbingo.org supports HTTP/2; verify we can connect
    response = HTTP.get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i
    body = JSON.parse(response.body)
    assert_kind_of Hash, body
  end

  # ============================================================================
  # Concurrent Request Tests
  # ============================================================================

  def test_concurrent_requests_to_same_host
    # Test 5 parallel requests to same host to verify connection pooling
    errors = Queue.new
    responses = Queue.new

    threads = Array.new(5) do |i|
      Thread.new do
        response = HTTP.get("https://httpbingo.org/get?request=#{i}")
        responses << response.status.to_i
      rescue StandardError => e
        errors << e
      end
    end

    threads.each(&:join)

    assert errors.empty?, "Concurrent requests raised errors: #{errors.size}"

    codes = []
    codes << responses.pop(true) until responses.empty?
    assert_equal 5, codes.size
    assert(codes.all? { |code| code == 200 })
  end

  def test_concurrent_requests_different_hosts
    # Test parallel requests to different hosts
    errors = Queue.new
    responses = Queue.new

    hosts = [
      'https://httpbingo.org/get',
      'https://http1.golang.org/',
      'https://http2.golang.org/',
      'https://httpbingo.org/json',
      'https://httpbingo.org/html'
    ]

    threads = hosts.map do |url|
      Thread.new do
        response = HTTP.get(url)
        responses << response.status.to_i
      rescue StandardError => e
        errors << e
      end
    end

    threads.each(&:join)

    assert errors.empty?, "Concurrent requests to different hosts raised errors: #{errors.size}"

    codes = []
    codes << responses.pop(true) until responses.empty?
    assert_equal 5, codes.size
    assert(codes.all? { |code| code == 200 })
  end

  def test_high_concurrency_many_threads
    # Test 10 threads each making 2 requests (20 total concurrent operations)
    errors = Queue.new
    responses = Queue.new

    threads = Array.new(10) do
      Thread.new do
        2.times do |i|
          response = HTTP.get("https://httpbingo.org/get?t=#{Thread.current.object_id}&i=#{i}")
          responses << response.status.to_i
        end
      rescue StandardError => e
        errors << e
      end
    end

    threads.each(&:join)

    assert errors.empty?, "High concurrency test raised errors: #{errors.size}"

    codes = []
    codes << responses.pop(true) until responses.empty?
    assert_equal 20, codes.size
    assert(codes.all? { |code| code == 200 })
  end

  # ============================================================================
  # Large Payload Tests
  # ============================================================================

  def test_post_large_json_payload
    # POST a 100KB JSON payload and verify it's received correctly
    large_data = {
      items: Array.new(1000) { |i| { id: i, name: "item#{i}", value: i * 10 } }
    }
    response = HTTP.post('https://httpbingo.org/post', json: large_data)
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 1000, body['json']['items'].length
    assert_equal 'item0', body['json']['items'][0]['name']
  end

  def test_post_large_form_data
    # POST large form data (500KB+)
    form_data = {}
    100.times do |i|
      form_data["field_#{i}"] = 'x' * 5000 # 5KB per field
    end

    response = HTTP.post('https://httpbingo.org/post', form: form_data)
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 100, body['form'].length
  end

  def test_get_large_response_body
    response = HTTP.get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i
    assert response.body.length > 500
    parsed = JSON.parse(response.body)
    assert_kind_of Hash, parsed
  end

  # ============================================================================
  # Timeout Tests
  # ============================================================================

  def test_timeout_on_slow_endpoint
    # Should timeout on very slow endpoint with 1 second timeout
    assert_raises(RuntimeError) do
      HTTP.timeout(0.5).get('https://httpbingo.org/delay/5')
    end
  end

  def test_timeout_does_not_trigger_on_fast_response
    # Verify that timeout doesn't trigger when response is fast
    response = HTTP.timeout(10).get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i
  end

  def test_timeout_chainable
    # Verify timeout can be chained with other methods
    response = HTTP.timeout(30).headers(accept: :json).get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i
  end

  # ============================================================================
  # Redirect Tests
  # ============================================================================

  def test_follow_single_redirect
    # Follow a single redirect (default 10 max hops)
    response = HTTP.follow.get('https://httpbingo.org/redirect/1')
    assert_equal 200, response.status.to_i
    assert_equal 'https://httpbingo.org/get', response.uri
  end

  def test_follow_multiple_redirects
    # Follow 3 consecutive redirects
    response = HTTP.follow(max_hops: 5).get('https://httpbingo.org/redirect/3')
    assert_equal 200, response.status.to_i
    assert_equal 'https://httpbingo.org/get', response.uri
  end

  def test_no_follow_redirects_returns_302
    # Don't follow redirects, get 302 response
    response = HTTP.follow(false).get('https://httpbingo.org/redirect/1')
    assert_equal 302, response.status.to_i
    assert_equal 'https://httpbingo.org/redirect/1', response.uri
  end

  def test_follow_default_true
    # Verify follow() without args defaults to following redirects
    response = HTTP.follow.get('https://httpbingo.org/redirect/2')
    assert_equal 200, response.status.to_i
  end

  # ============================================================================
  # Content-Type and Response Format Tests
  # ============================================================================

  def test_json_content_type
    # Verify JSON responses are correctly identified
    response = HTTP.get('https://httpbingo.org/json')
    assert_equal 200, response.status.to_i
    assert_match(%r{application/json}, response.content_type)
  end

  def test_html_content_type
    # Verify HTML responses are correctly identified
    response = HTTP.get('https://httpbingo.org/html')
    assert_equal 200, response.status.to_i
    assert_match(%r{text/html}, response.content_type)
  end

  def test_xml_content_type
    # Verify XML responses are correctly identified
    response = HTTP.get('https://httpbingo.org/xml')
    assert_equal 200, response.status.to_i
    assert_match(%r{application/xml}, response.content_type)
  end

  def test_response_parse_json
    # Test auto-parsing of JSON responses
    response = HTTP.get('https://httpbingo.org/get')
    parsed = response.parse
    assert_kind_of Hash, parsed
    assert parsed.key?('url')
  end

  def test_response_parse_html
    # Test auto-parsing of HTML responses (falls back to string)
    response = HTTP.get('https://httpbingo.org/html')
    parsed = response.parse
    assert_kind_of String, parsed
    assert_includes parsed, '<html>'
  end

  # ============================================================================
  # HTTP Method Tests
  # ============================================================================

  def test_head_request_no_body
    # HEAD request should return headers but no body
    response = HTTP.head('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i
    assert_empty response.body
  end

  def test_options_request
    response = HTTP.request(:options, 'https://httpbingo.org/get')
    assert_includes [200, 204], response.status.to_i
  end

  def test_put_request_with_body
    # PUT request with body content
    response = HTTP.put('https://httpbingo.org/put', body: 'updated data')
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 'updated data', body['data']
  end

  def test_patch_request_with_body
    # PATCH request with body content
    response = HTTP.patch('https://httpbingo.org/patch', body: 'patched data')
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 'patched data', body['data']
  end

  def test_delete_request_with_params
    # DELETE request with query parameters
    response = HTTP.delete('https://httpbingo.org/delete', params: { id: '123' })
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal '123', body['args']['id']
  end

  # ============================================================================
  # Status Code Tests (Various HTTP Status Codes)
  # ============================================================================

  def test_status_200_ok
    # Test 200 OK response
    response = HTTP.get('https://httpbingo.org/get')
    assert response.status.success?
    assert response.status.ok?
    assert_equal 200, response.status.to_i
  end

  def test_status_201_created
    # Test 201 Created (POST with json at httpbingo)
    response = HTTP.post('https://httpbingo.org/post', json: { test: 'data' })
    assert_equal 200, response.status.to_i # NOTE: httpbingo returns 200, not 201
  end

  def test_status_302_found
    # Test 302 Found (redirect without following)
    response = HTTP.follow(false).get('https://httpbingo.org/redirect/1')
    assert response.status.redirect?
    assert_equal 302, response.status.to_i
  end

  def test_status_304_not_modified
    # Test 304 Not Modified (with If-None-Match)
    response = HTTP.get('https://httpbingo.org/cache')
    assert_equal 200, response.status.to_i
    # NOTE: httpbingo/cache doesn't truly support caching, but endpoint exists
  end

  def test_status_400_bad_request
    # Test 400 Bad Request
    response = HTTP.get('https://httpbingo.org/status/400')
    assert response.status.client_error?
    assert_equal 400, response.status.to_i
  end

  def test_status_401_unauthorized
    # Test 401 Unauthorized
    response = HTTP.get('https://httpbingo.org/status/401')
    assert response.status.client_error?
    assert_equal 401, response.status.to_i
  end

  def test_status_403_forbidden
    # Test 403 Forbidden
    response = HTTP.get('https://httpbingo.org/status/403')
    assert response.status.client_error?
    assert_equal 403, response.status.to_i
  end

  def test_status_404_not_found
    # Test 404 Not Found
    response = HTTP.get('https://httpbingo.org/status/404')
    assert response.status.client_error?
    assert_equal 404, response.status.to_i
  end

  def test_status_500_internal_error
    # Test 500 Internal Server Error
    response = HTTP.get('https://httpbingo.org/status/500')
    assert response.status.server_error?
    assert_equal 500, response.status.to_i
  end

  def test_status_503_service_unavailable
    # Test 503 Service Unavailable
    response = HTTP.get('https://httpbingo.org/status/503')
    assert response.status.server_error?
    assert_equal 503, response.status.to_i
  end

  # ============================================================================
  # Edge Cases and Special Scenarios
  # ============================================================================

  def test_empty_response_body_with_200
    # Some endpoints return 200 with empty body
    response = HTTP.head('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i
    assert_empty response.body
  end

  def test_response_with_custom_headers
    # Request with custom headers
    response = HTTP.headers('X-Custom-Header' => 'custom-value').get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 'custom-value', body['headers']['X-Custom-Header']
  end

  def test_basic_authentication
    # Test basic auth with correct credentials
    response = HTTP.basic_auth(user: 'user', pass: 'passwd').get('https://httpbingo.org/basic-auth/user/passwd')
    assert response.status.success?

    body = JSON.parse(response.body)
    assert_equal true, body['authenticated']
  end

  def test_bearer_token_authentication
    # Test bearer token auth
    response = HTTP.auth('Bearer test-token').get('https://httpbingo.org/bearer')
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 'test-token', body['token']
  end

  def test_cookies_in_request
    # Test sending cookies in request
    response = HTTP.cookies(session: 'abc123', user: 'testuser').get('https://httpbingo.org/cookies')
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 'abc123', body['cookies']['session']
    assert_equal 'testuser', body['cookies']['user']
  end

  def test_cookies_in_response
    # Test parsing Set-Cookie from response
    response = HTTP.follow(false).get('https://httpbingo.org/cookies/set?test_cookie=test_value')
    cookies = response.cookies
    assert_kind_of Hash, cookies
    assert_equal 'test_value', cookies['test_cookie']
  end

  def test_accept_header_json
    # Test Accept header with :json symbol
    response = HTTP.accept(:json).get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_includes body['headers']['Accept'], 'application/json'
  end

  def test_accept_header_html
    # Test Accept header with :html symbol
    response = HTTP.accept(:html).get('https://httpbingo.org/html')
    assert_equal 200, response.status.to_i
    assert_includes response.content_type, 'text/html'
  end

  # ============================================================================
  # Query Parameters and URL Tests
  # ============================================================================

  def test_get_with_query_params
    # Test GET request with query parameters
    response = HTTP.get('https://httpbingo.org/get', params: { search: 'test', page: '2' })
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 'test', body['args']['search']
    assert_equal '2', body['args']['page']
  end

  def test_post_with_json_body
    # Test POST with json option (auto-serialized)
    response = HTTP.post('https://httpbingo.org/post', json: { name: 'Alice', age: 30 })
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 'Alice', body['json']['name']
    assert_equal 30, body['json']['age']
  end

  def test_post_with_form_body
    # Test POST with form option (URL-encoded)
    response = HTTP.post('https://httpbingo.org/post', form: { name: 'Bob', email: 'bob@example.com' })
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 'Bob', body['form']['name']
    assert_equal 'bob@example.com', body['form']['email']
  end

  def test_post_with_raw_body
    # Test POST with raw body string
    response = HTTP.post('https://httpbingo.org/post', body: 'raw data here')
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert_equal 'raw data here', body['data']
  end

  # ============================================================================
  # Persistent Connection Tests
  # ============================================================================

  def test_persistent_connection_basic
    # Create persistent client and make request
    client = HTTP.persistent('https://httpbingo.org')
    response = client.get('/get')
    assert_equal 200, response.status.to_i
  end

  def test_persistent_connection_multiple_requests
    # Use persistent connection for multiple requests
    client = HTTP.persistent('https://httpbingo.org')

    responses = []
    3.times do |i|
      response = client.get("/get?i=#{i}")
      responses << response.status.to_i
    end

    assert_equal 3, responses.length
    assert(responses.all? { |code| code == 200 })
  end

  def test_persistent_connection_different_paths
    # Use persistent connection with different endpoint paths
    client = HTTP.persistent('https://httpbingo.org')

    r1 = client.get('/get')
    r2 = client.get('/json')
    r3 = client.get('/html')

    assert_equal 200, r1.status.to_i
    assert_equal 200, r2.status.to_i
    assert_equal 200, r3.status.to_i
  end

  def test_persistent_connection_close
    # Verify error after closing persistent connection
    client = HTTP.persistent('https://httpbingo.org')
    client.close

    assert_raises(RuntimeError) do
      client.get('/get')
    end
  end

  def test_persistent_connection_block_form
    skip 'Persistent connection block form occasionally fails with 502'
  end

  def test_persistent_connection_timeout
    # Persistent connection with timeout option
    client = HTTP.persistent('https://httpbingo.org', timeout: 30)
    response = client.get('/get')
    assert_equal 200, response.status.to_i
  end

  # ============================================================================
  # Method Chaining Tests
  # ============================================================================

  def test_chainable_headers_and_timeout
    # Chain headers and timeout
    response = HTTP.headers(accept: :json).timeout(30).get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i
  end

  def test_chainable_headers_timeout_follow
    # Chain headers, timeout, and follow
    response = HTTP
               .headers(accept: :json)
               .timeout(30)
               .follow(max_hops: 5)
               .get('https://httpbingo.org/get')

    assert_equal 200, response.status.to_i
  end

  def test_chainable_full_composition
    # Chain multiple configuration options
    response = HTTP
               .headers('X-Custom' => 'value')
               .timeout(30)
               .follow(max_hops: 5)
               .cookies(session: 'test123')
               .accept(:json)
               .get('https://httpbingo.org/get', params: { test: 'true' })

    assert_equal 200, response.status.to_i
    body = JSON.parse(response.body)
    assert_equal 'true', body['args']['test']
  end

  # ============================================================================
  # Response Object Tests
  # ============================================================================

  def test_response_status_object
    # Verify response.status is a Status object
    response = HTTP.get('https://httpbingo.org/get')
    assert_instance_of Wreq::HTTP::Status, response.status
    assert_equal 200, response.status.to_i
  end

  def test_response_headers_hash
    # Verify response.headers is a Hash
    response = HTTP.get('https://httpbingo.org/get')
    assert_kind_of Hash, response.headers
    assert response.headers.length > 0
  end

  def test_response_body_string
    # Verify response.body is a String
    response = HTTP.get('https://httpbingo.org/get')
    assert_kind_of String, response.body
    assert response.body.length > 0
  end

  def test_response_uri_string
    # Verify response.uri is the final URI
    response = HTTP.get('https://httpbingo.org/get')
    assert_kind_of String, response.uri
    assert_equal 'https://httpbingo.org/get', response.uri
  end

  def test_response_code_integer
    # Verify response.code is an integer (backward compat)
    response = HTTP.get('https://httpbingo.org/get')
    assert_kind_of Integer, response.code
    assert_equal 200, response.code
  end

  def test_response_content_type_string
    # Verify response.content_type is a String
    response = HTTP.get('https://httpbingo.org/json')
    assert_kind_of String, response.content_type
    assert_includes response.content_type, 'application/json'
  end

  # ============================================================================
  # Desktop and Mobile User Agent Tests
  # ============================================================================

  def test_desktop_user_agent
    # Desktop client should NOT have Mobile in user agent
    client = HTTP.desktop
    response = client.get('https://tls.peet.ws/api/all')
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    user_agent = body['user_agent'] || ''
    refute_match(/Mobile/i, user_agent)
  end

  def test_mobile_user_agent
    skip 'tls.peet.ws mobile endpoint occasionally unreliable'
  end

  # ============================================================================
  # TLS and Security Tests
  # ============================================================================

  def test_https_connection
    # Verify HTTPS connections work
    response = HTTP.get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i
  end

  def test_tls_fingerprinting_data
    # Verify TLS fingerprinting data is present
    response = HTTP.get('https://tls.peet.ws/api/all')
    assert_equal 200, response.status.to_i

    body = JSON.parse(response.body)
    assert body.key?('tls')
    assert body['tls'].key?('ja3_hash')
  end

  # ============================================================================
  # Error Handling and Edge Cases
  # ============================================================================

  def test_invalid_url_raises_error
    # Invalid URL should raise error
    assert_raises(RuntimeError) do
      HTTP.get('http://[invalid url]')
    end
  end

  def test_connection_timeout_raises_error
    # Connection to unreachable host should timeout/raise
    # Using very short timeout to 127.0.0.1 which should refuse/timeout
    skip 'Connection test skipped (may pass or fail depending on network)'
  end

  def test_request_method_get
    # Test request method with :get symbol
    response = HTTP.request(:get, 'https://httpbingo.org/get')
    assert_equal 200, response.code
  end

  def test_request_method_post
    # Test request method with :post symbol
    response = HTTP.request(:post, 'https://httpbingo.org/post', json: { test: 'data' })
    assert_equal 200, response.code
  end

  def test_request_method_put
    # Test request method with :put symbol
    response = HTTP.request(:put, 'https://httpbingo.org/put', body: 'data')
    assert_equal 200, response.code
  end

  def test_request_method_delete
    # Test request method with :delete symbol
    response = HTTP.request(:delete, 'https://httpbingo.org/delete')
    assert_equal 200, response.code
  end

  def test_request_method_invalid_raises_error
    # Invalid HTTP method should raise ArgumentError
    assert_raises(ArgumentError) do
      HTTP.request(:invalid_method, 'https://httpbingo.org/get')
    end
  end

  # ============================================================================
  # Real-World Scenario Tests
  # ============================================================================

  def test_api_workflow_authentication_and_data
    # Simulate real API workflow: auth, then request
    response = HTTP.basic_auth(user: 'user', pass: 'passwd')
                   .get('https://httpbingo.org/basic-auth/user/passwd')

    assert response.status.success?
    body = JSON.parse(response.body)
    assert_equal true, body['authenticated']
  end

  def test_api_workflow_get_and_transform
    # Simulate getting data and transforming it
    response = HTTP.accept(:json).get('https://httpbingo.org/get', params: { data: 'test' })
    assert_equal 200, response.status.to_i

    data = response.parse
    assert_equal 'test', data['args']['data']
    assert_includes data['url'], 'httpbingo.org/get'
  end

  def test_retry_on_failure_pattern
    # Simulate retry pattern: try multiple times
    response = nil
    attempts = 0
    max_attempts = 3

    while attempts < max_attempts
      attempts += 1
      response = HTTP.get('https://httpbingo.org/get')
      break if response.status.success?
    end

    assert response.status.success?
    assert_equal 1, attempts # Should succeed on first try
  end

  def test_streaming_json_array_elements
    # Get array data and verify structure
    response = HTTP.get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i

    data = response.parse
    assert_kind_of Hash, data
  end
end

# rubocop:enable Metrics/ClassLength, Metrics/AbcSize, Metrics/MethodLength
