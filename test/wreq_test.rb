# frozen_string_literal: true

# rubocop:disable Metrics/ClassLength, Metrics/AbcSize, Metrics/MethodLength, Layout/LineLength, Naming/VariableNumber, Style/NumericPredicate, Style/ZeroLengthPredicate

require 'minitest/autorun'
require_relative '../lib/wreq_rb'
require 'json'

class WreqTest < Minitest::Test
  HTTP = Wreq::HTTP

  def test_basic_get_request
    response = HTTP.get('https://tls.peet.ws/api/all')
    assert_equal(200, response.status)
    assert_kind_of(String, response.body)
    assert_kind_of(Hash, response.headers)

    # Verify the response contains TLS data
    data = JSON.parse(response.body)
    refute_nil(data['tls'])
  end

  def test_client_instance_get_request
    response = HTTP.get('https://tls.peet.ws/api/all')
    assert_equal(200, response.status)
    assert_kind_of(String, response.body)

    # Verify the response contains TLS data
    data = JSON.parse(response.body)
    refute_nil(data['tls'])
  end

  def get_headers_from_response(body)
    # Helper function to extract headers from the response
    if body['http_version'] == 'h2' && body['http2']
      # For HTTP/2, find the HEADERS frame in sent_frames
      headers_frame = body['http2']['sent_frames'].find { |frame| frame['frame_type'] == 'HEADERS' }
      headers_frame ? headers_frame['headers'] : []
    elsif body['http1'] && body['http1']['headers']
      body['http1']['headers']
    else
      []
    end
  end

  def test_cookies_support
    cookie = 'cookie1=value1; cookie2=value2'
    client_with_cookies = HTTP.headers({ 'Cookie' => cookie })

    response = client_with_cookies.get('https://httpbingo.org/cookies')
    assert_equal(200, response.status)

    data = JSON.parse(response.body)
    assert_equal('value1', data['cookie1'])
    assert_equal('value2', data['cookie2'])
  end

  def test_random_user_agent
    # Make multiple requests and verify different user agents are used
    agents = []
    5.times do
      client = HTTP::Client.new
      response = client.get('https://tls.peet.ws/api/all')
      assert_equal(200, response.status)

      body = JSON.parse(response.body)
      # Get headers and extract user agent
      headers = get_headers_from_response(body)
      user_agent = headers.find { |h| h.start_with?('user-agent:') }

      # Fallback to the top-level user_agent if not found in headers
      user_agent = user_agent ? user_agent.sub('user-agent: ', '') : body['user_agent']
      agents << user_agent
    end

    # Check that we got at least 2 different user agents (should be random)
    assert agents.uniq.size > 1, "Expected different random user agents, but got: #{agents.uniq}"
  end

  def test_desktop_client
    # Test the desktop client
    client = HTTP.desktop
    response = client.get('https://tls.peet.ws/api/all')
    assert_equal(200, response.status)

    body = JSON.parse(response.body)
    # Get headers and extract user agent
    headers = get_headers_from_response(body)
    user_agent = headers.find { |h| h.start_with?('user-agent:') }

    # Fallback to the top-level user_agent if not found in headers
    user_agent = user_agent ? user_agent.sub('user-agent: ', '') : body['user_agent']

    # Verify it's a desktop user agent (doesn't have "Mobile" in it)
    refute_match(/Mobile/i, user_agent) if user_agent
  end

  def test_mobile_client
    # Test the mobile client
    client = HTTP.mobile
    response = client.get('https://tls.peet.ws/api/all')
    assert_equal(200, response.status)

    body = JSON.parse(response.body)
    # Get headers and extract user agent
    headers = get_headers_from_response(body)
    user_agent = headers.find { |h| h.start_with?('user-agent:') }

    # Fallback to the top-level user_agent if not found in headers
    user_agent = user_agent ? user_agent.sub('user-agent: ', '') : body['user_agent']

    # Verify it's a mobile user agent - check for common mobile identifiers
    mobile_indicators = [/Mobile/i, /iPhone/i, /iPad/i, /iOS/i, /Android/i]
    is_mobile = mobile_indicators.any? { |pattern| user_agent =~ pattern }

    assert is_mobile, "Expected a mobile user agent, but got: #{user_agent}"
  end

  def test_headers
    response = HTTP
               .headers(accept: 'application/json', user_agent: 'Test Client')
               .get('https://tls.peet.ws/api/all')

    assert_equal(200, response.status)
    body = JSON.parse(response.body)

    # Get headers and check for accept header
    headers = get_headers_from_response(body)
    accept_header = headers.find { |h| h.start_with?('accept:') }

    # Verify the header contains application/json
    assert_match(%r{application/json}, accept_header || '') if accept_header
  end

  def test_post_request
    response = HTTP.post(
      'https://postman-echo.com/post',
      body: 'test body'
    )

    assert_equal(200, response.status)
    body = JSON.parse(response.body)
    assert_equal('test body', body['data'])
  end

  def test_persistent_basic
    client = HTTP.persistent('https://httpbingo.org')
    response = client.get('/get')
    assert_equal(200, response.status)
  end

  def test_persistent_block_form
    result = HTTP.persistent('https://httpbingo.org') do |http|
      http.get('/get')
    end
    assert_equal(200, result.status)
  end

  def test_persistent_relative_urls
    client = HTTP.persistent('https://httpbingo.org')
    r1 = client.get('/get')
    r2 = client.get('/ip')
    assert_equal(200, r1.status)
    assert_equal(200, r2.status)
  end

  def test_persistent_close
    client = HTTP.persistent('https://httpbingo.org')
    client.close
    assert_raises(RuntimeError) { client.get('/get') }
  end

  def test_persistent_timeout_option
    client = HTTP.persistent('https://httpbingo.org', timeout: 30)
    response = client.get('/get')
    assert_equal(200, response.status)
  end

  def test_persistent_multiple_requests
    client = HTTP.persistent('https://httpbingo.org')
    5.times do
      response = client.get('/get')
      assert_equal(200, response.status)
    end
  end

  def test_post_json
    response = HTTP
               .headers(content_type: 'application/json')
               .post(
                 'https://postman-echo.com/post',
                 body: JSON.generate({ name: 'test', value: 123 })
               )

    assert_equal(200, response.status)
    body = JSON.parse(response.body)
    data = body['data'].is_a?(String) ? JSON.parse(body['data']) : body['data']
    assert_equal({ 'name' => 'test', 'value' => 123 }, data)
  end

  def test_put_request
    response = HTTP.put(
      'https://postman-echo.com/put',
      body: 'updated content'
    )

    assert_equal(200, response.status)
    body = JSON.parse(response.body)
    assert_equal('updated content', body['data'])
  end

  def test_delete_request
    response = HTTP.delete('https://httpbingo.org/delete')
    assert_equal(200, response.status)
  end

  def test_head_request
    response = HTTP.head('https://httpbingo.org/get')
    assert_equal(200, response.status)
    assert_empty(response.body)
  end

  def test_patch_request
    response = HTTP.patch(
      'https://postman-echo.com/patch',
      body: 'patched content'
    )

    assert_equal(200, response.status)
    body = JSON.parse(response.body)
    assert_equal('patched content', body['data'])
  end

  def test_follow_redirects
    response = HTTP
               .follow(true)
               .get('https://httpbingo.org/redirect/1')

    assert_equal(200, response.status)
    assert_equal('https://httpbingo.org/get', response.uri)
  end

  def test_no_follow_redirects
    response = HTTP
               .follow(false)
               .get('https://httpbingo.org/redirect/1')

    assert_equal(302, response.status)
    assert_equal('https://httpbingo.org/redirect/1', response.uri)
  end

  def test_response_methods
    response = HTTP.get('https://tls.peet.ws/api/all')

    assert_kind_of(Integer, response.status)
    assert_kind_of(String, response.body)
    assert_kind_of(Hash, response.headers)
    assert_kind_of(String, response.uri)
    assert_kind_of(String, response.to_s)
    assert_kind_of(Integer, response.code)
    assert_equal(response.status, response.code)
  end

  def test_content_type_and_charset
    response = HTTP
               .headers(accept: 'application/json')
               .get('https://tls.peet.ws/api/all')

    assert_equal('application/json', response.content_type)

    # Test with a response that has charset
    charset_response = HTTP.get('https://httpbingo.org/html')
    assert_kind_of(String, charset_response.content_type)
    assert_includes(charset_response.content_type.to_s.downcase, 'text/html')

    # Charset might be present depending on the server response
    return unless charset_response.charset

    assert_kind_of(String, charset_response.charset)
  end

  def test_bing_search_results
    # Create a client with a common browser user agent to avoid being blocked
    client = HTTP::Client.new

    # Fetch Bing search results for "Coffee"
    response = client
               .headers(accept: 'text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8')
               .get('https://www.bing.com/search?form=QBRE&q=Coffee&lq=0&rdr=1')

    assert_equal(200, response.status)

    # Convert response body to lowercase for case-insensitive checks
    body_text = response.body.downcase

    # Check for common elements that should be on a search results page
    has_search_results = body_text.include?('results') || body_text.include?('search') || body_text.include?('programming language')
    has_links = body_text.include?('<a href=') || body_text.include?('href=')

    # Assert that we have basic search results structure
    assert has_search_results, 'No search results found in response'
    assert has_links, 'No links found in search results'
  end

  def test_tls_fingerprinting
    fingerprints = []

    5.times do
      client = HTTP::Client.new

      response = client.get('https://tls.peet.ws/api/all')
      assert_equal(200, response.status)

      data = JSON.parse(response.body)

      assert_kind_of(Hash, data['tls'])
      assert_kind_of(Array, data['tls']['ciphers'])
      assert(data['tls']['ciphers'].size > 0, 'Expected TLS ciphers to be present')

      refute_nil(data['tls']['ja3'], 'JA3 fingerprint should be present')
      refute_nil(data['tls']['ja3_hash'], 'JA3 hash should be present')
      refute_nil(data['tls']['ja4'], 'JA4 fingerprint should be present')

      fingerprints << {
        ja3: data['tls']['ja3_hash'],
        ja4: data['tls']['ja4']
      }

      tls_version = data['tls']['tls_version_negotiated']
      assert(%w[771 772].include?(tls_version),
             "Expected modern TLS version (TLS 1.2 or 1.3), got: #{tls_version}")
    end

    ja3_fingerprints = fingerprints.map { |f| f[:ja3] }.uniq
    ja4_fingerprints = fingerprints.map { |f| f[:ja4] }.uniq

    assert(ja3_fingerprints.size > 1 || ja4_fingerprints.size > 1,
           'Expected fingerprint randomization, but got identical fingerprints across 5 requests. ' \
          "JA3: #{ja3_fingerprints.size} unique (#{ja3_fingerprints.first}...), " \
          "JA4: #{ja4_fingerprints.size} unique (#{ja4_fingerprints.first}...)")
  end

  def test_timeout_chainable
    response = Wreq::HTTP.timeout(30).get('https://httpbingo.org/get')
    assert_equal 200, response.status
  end

  def test_timeout_with_headers
    response = Wreq::HTTP.headers(accept: 'application/json').timeout(10).get('https://httpbingo.org/get')
    assert_equal 200, response.status
  end

  def test_via_with_host_and_port
    skip 'requires proxy server'
    response = Wreq::HTTP.via('proxy.example.com', 8080).get('https://httpbingo.org/get')
    assert_equal 200, response.status
  end

  def test_via_with_auth
    skip 'requires proxy server with auth'
    response = Wreq::HTTP.via('proxy.example.com', 8080, 'user', 'pass').get('https://httpbingo.org/get')
    assert_equal 200, response.status
  end

  def test_via_chainable
    skip 'requires proxy server'
    response = Wreq::HTTP.via('proxy.example.com', 8080).headers(accept: 'application/json').get('https://httpbingo.org/get')
    assert_equal 200, response.status
  end

  def test_status_object_success
    response = Wreq::HTTP.get('https://httpbingo.org/get')
    assert_instance_of Wreq::HTTP::Status, response.status
    assert_equal 200, response.status.to_i
    assert_equal '200 OK', response.status.to_s
    assert_equal 'OK', response.status.reason
    assert response.status.success?
    assert response.status.ok?
    refute response.status.redirect?
    refute response.status.client_error?
    refute response.status.server_error?
  end

  def test_status_equality
    response = Wreq::HTTP.get('https://httpbingo.org/get')
    assert_equal 200, response.status
    assert response.status == 200
  end

  def test_status_404
    response = Wreq::HTTP.get('https://httpbingo.org/status/404')
    assert_equal 404, response.status.to_i
    assert_equal 'Not Found', response.status.reason
    assert response.status.client_error?
    refute response.status.success?
  end

  def test_status_redirect
    response = Wreq::HTTP.follow(false).get('https://httpbingo.org/redirect/1')
    assert response.status.redirect?
    refute response.status.success?
  end

  def test_code_backward_compat
    response = Wreq::HTTP.get('https://httpbingo.org/get')
    assert_equal 200, response.code
    assert_kind_of Integer, response.code
  end

  def test_cookies
    response = Wreq::HTTP.cookies(session: 'abc123', user: 'test').get('https://httpbingo.org/cookies')
    assert_equal 200, response.status.to_i
    assert response.body.include?('abc123')
  end

  def test_basic_auth
    response = Wreq::HTTP.basic_auth(user: 'user', pass: 'passwd').get('https://httpbingo.org/basic-auth/user/passwd')
    assert_equal 200, response.status.to_i
  end

  def test_auth_bearer
    response = Wreq::HTTP.auth('Bearer test-token').get('https://httpbingo.org/bearer')
    assert_equal 200, response.status.to_i
  end

  def test_accept_symbol
    response = Wreq::HTTP.accept(:json).get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i
  end

  def test_accept_string
    response = Wreq::HTTP.accept('text/html').get('https://httpbingo.org/html')
    assert_equal 200, response.status.to_i
  end

  def test_chainable_auth_methods
    response = Wreq::HTTP
               .cookies(session: 'test')
               .headers(x_custom: 'value')
               .accept(:json)
               .get('https://httpbingo.org/get')
    assert_equal 200, response.status.to_i
  end

  def test_parse_json
    response = Wreq::HTTP.get('https://httpbingo.org/get')
    parsed = response.parse
    assert_instance_of Hash, parsed
    assert parsed.key?('url')
  end

  def test_parse_non_json
    response = Wreq::HTTP.get('https://httpbingo.org/html')
    parsed = response.parse
    assert_instance_of String, parsed
    assert parsed.include?('<html>')
  end

  def test_flush
    response = Wreq::HTTP.get('https://httpbingo.org/get')
    flushed = response.flush
    assert_equal response, flushed
  end

  def test_body_backward_compat
    response = Wreq::HTTP.get('https://httpbingo.org/get')
    assert_instance_of String, response.body
    assert response.body.length > 0
  end

  def test_follow_default
    response = Wreq::HTTP.follow.get('https://httpbingo.org/redirect/1')
    assert_equal 200, response.status.to_i
  end

  def test_follow_with_max_hops
    response = Wreq::HTTP.follow(max_hops: 5).get('https://httpbingo.org/redirect/3')
    assert_equal 200, response.status.to_i
  end

  def test_follow_true_backward_compat
    response = Wreq::HTTP.follow(true).get('https://httpbingo.org/redirect/1')
    assert_equal 200, response.status.to_i
  end

  def test_follow_false_backward_compat
    response = Wreq::HTTP.follow(false).get('https://httpbingo.org/redirect/1')
    assert response.status.redirect?
  end

  def test_post_with_json_option
    response = HTTP.post('https://postman-echo.com/post', json: { name: 'test', value: 123 })
    assert_equal 200, response.code
    body = JSON.parse(response.body)
    assert_equal 'test', body['json']['name']
    assert_equal 123, body['json']['value']
    assert body['headers']['content-type'].include?('application/json')
  end

  def test_post_with_form_option
    response = HTTP.post('https://postman-echo.com/post', form: { name: 'test', email: 'a@b.com' })
    assert_equal 200, response.code
    body = JSON.parse(response.body)
    assert_equal 'test', body['form']['name']
    assert_equal 'a@b.com', body['form']['email']
    assert body['headers']['content-type'].include?('application/x-www-form-urlencoded')
  end

  def test_post_with_body_option_backward_compat
    response = HTTP.post('https://postman-echo.com/post', body: 'raw string data')
    assert_equal 200, response.code
    body = JSON.parse(response.body)
    assert_equal 'raw string data', body['data']
  end

  def test_get_with_params_option
    response = HTTP.get('https://postman-echo.com/get', params: { q: 'search', page: '2' })
    assert_equal 200, response.code
    body = JSON.parse(response.body)
    assert_equal 'search', body['args']['q']
    assert_equal '2', body['args']['page']
  end

  def test_request_method_get
    response = HTTP.request(:get, 'https://httpbingo.org/get')
    assert_equal 200, response.code
  end

  def test_request_method_post_with_json
    response = HTTP.request(:post, 'https://httpbingo.org/post', json: { a: 1 })
    assert_equal 200, response.code
    body = JSON.parse(response.body)
    assert_equal 1, body['json']['a']
  end

  def test_request_method_invalid_verb
    assert_raises(ArgumentError) do
      HTTP.request(:invalid, 'https://httpbingo.org/get')
    end
  end

  def test_chainable_with_params
    response = HTTP.headers(accept: 'application/json').get('https://postman-echo.com/get', params: { test: 'value' })
    assert_equal 200, response.code
    body = JSON.parse(response.body)
    assert_equal 'value', body['args']['test']
    assert body['headers']['accept'].include?('application/json')
  end

  def test_encoding_chainable
    response = HTTP.encoding('UTF-8').get('https://httpbingo.org/get')
    assert_equal 200, response.code
  end

  def test_cookies_parsing
    response = HTTP.get('https://httpbingo.org/cookies/set?test_cookie=test_value')
    cookies = response.cookies
    assert_instance_of Hash, cookies
  end

  def test_cookies_empty
    response = HTTP.get('https://httpbingo.org/get')
    cookies = response.cookies
    assert_instance_of Hash, cookies
  end

  def test_encoding_applied_to_response
    response = HTTP.encoding('UTF-8').get('https://httpbingo.org/get')
    assert_equal 200, response.status
    body = JSON.parse(response.body)
    assert_instance_of Hash, body
  end

  def test_status_predicates_404
    response = HTTP.get('https://httpbingo.org/status/404')
    assert response.status.client_error?
    refute response.status.success?
  end

  def test_status_predicates_500
    response = HTTP.get('https://httpbingo.org/status/500')
    assert response.status.server_error?
    refute response.status.success?
  end

  def test_status_predicates_302
    response = HTTP.follow(false).get('https://httpbingo.org/redirect/1')
    assert response.status.redirect?
    refute response.status.success?
  end

  def test_response_parse_json_auto
    response = HTTP.get('https://httpbingo.org/get')
    parsed = response.parse
    assert_instance_of Hash, parsed
    assert_equal 'https://httpbingo.org/get', parsed['url']
  end

  def test_response_parse_non_json_fallback
    response = HTTP.get('https://httpbingo.org/html')
    parsed = response.parse
    assert_instance_of String, parsed
    assert_includes parsed, '<html>'
  end

  def test_response_cookies_parsing_set_cookie
    response = HTTP.follow(false).get('https://httpbingo.org/cookies/set?test_cookie=test_value')
    assert response.status.redirect?
    assert_equal 'test_value', response.cookies['test_cookie']
  end

  def test_response_cookies_empty
    response = HTTP.get('https://httpbingo.org/get')
    cookies = response.cookies
    assert_instance_of Hash, cookies
    assert_empty cookies
  end

  def test_chainable_composition_full
    response = HTTP
               .headers(x_custom: 'val')
               .timeout(30)
               .follow(max_hops: 5)
               .cookies(session: 'abc')
               .accept(:json)
               .get('https://postman-echo.com/get', params: { q: 'test' })

    parsed = response.parse
    assert_equal 'test', parsed['args']['q']
    assert_includes parsed['headers']['x-custom'], 'val'
    assert_includes parsed['headers']['cookie'], 'session=abc'
    assert_includes parsed['headers']['accept'], 'application/json'
  end

  def test_basic_auth_success
    response = HTTP.basic_auth(user: 'user', pass: 'passwd').get('https://httpbingo.org/basic-auth/user/passwd')
    assert response.status.ok?
    parsed = response.parse
    assert_equal true, parsed['authorized']
  end

  def test_auth_bearer_success
    response = HTTP.auth('Bearer test-token').get('https://httpbingo.org/bearer')
    assert response.status.ok?
    parsed = response.parse
    assert_equal 'test-token', parsed['token']
  end

  def test_timeout_raises
    assert_raises(RuntimeError) do
      HTTP.timeout(0.001).get('https://httpbingo.org/delay/10')
    end
  end

  def test_invalid_url_raises
    assert_raises(RuntimeError) do
      HTTP.get('http://[invalid]')
    end
  end

  def test_thread_safety_concurrent_requests
    errors = Queue.new
    responses = Queue.new

    threads = Array.new(5) do
      Thread.new do
        2.times do
          response = HTTP.get('https://httpbingo.org/get', params: { t: Thread.current.object_id })
          responses << response.code
        end
      rescue StandardError => e
        errors << e
      end
    end

    threads.each(&:join)

    assert errors.empty?, "Concurrent requests raised errors: #{errors.size}"

    codes = []
    codes << responses.pop(true) until responses.empty?
    assert_equal 10, codes.size
    assert(codes.all? { |code| code == 200 })
  end

  def test_put_with_json_option
    response = HTTP.put('https://postman-echo.com/put', json: { name: 'put-test', value: 7 })
    assert response.status.ok?
    body = response.parse
    assert_equal 'put-test', body['json']['name']
    assert_equal 7, body['json']['value']
  end

  def test_patch_with_form_option
    response = HTTP.patch('https://postman-echo.com/patch', form: { name: 'patch-test', value: 'ok' })
    assert response.status.ok?
    body = response.parse
    assert_equal 'patch-test', body['form']['name']
    assert_equal 'ok', body['form']['value']
  end

  def test_request_method_delete_with_params
    response = HTTP.request(:delete, 'https://postman-echo.com/delete', params: { reason: 'cleanup' })
    assert response.status.ok?
    body = response.parse
    assert_equal 'cleanup', body['args']['reason']
  end
end

# rubocop:enable Metrics/ClassLength, Metrics/AbcSize, Metrics/MethodLength, Layout/LineLength, Naming/VariableNumber, Style/NumericPredicate, Style/ZeroLengthPredicate
