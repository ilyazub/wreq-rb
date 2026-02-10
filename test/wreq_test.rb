require 'minitest/autorun'
require_relative '../lib/wreq_rb'
require 'json'

class WreqTest < Minitest::Test
  HTTP = Wreq::HTTP

  def test_basic_get_request
    response = HTTP.get("https://tls.peet.ws/api/all")
    assert_equal(200, response.status)
    assert_kind_of(String, response.body)
    assert_kind_of(Hash, response.headers)
    
    # Verify the response contains TLS data
    data = JSON.parse(response.body)
    refute_nil(data["tls"])
  end

  def test_client_instance_get_request
    response = HTTP.get("https://tls.peet.ws/api/all")
    assert_equal(200, response.status)
    assert_kind_of(String, response.body)
    
    # Verify the response contains TLS data
    data = JSON.parse(response.body)
    refute_nil(data["tls"])
  end

  def get_headers_from_response(body)
    # Helper function to extract headers from the response
    if body["http_version"] == "h2" && body["http2"]
      # For HTTP/2, find the HEADERS frame in sent_frames
      headers_frame = body["http2"]["sent_frames"].find { |frame| frame["frame_type"] == "HEADERS" }
      return headers_frame ? headers_frame["headers"] : []
    elsif body["http1"] && body["http1"]["headers"]
      return body["http1"]["headers"]
    else
      return []
    end
  end

  def test_cookies_support
    cookie = "cookie1=value1; cookie2=value2"
    client_with_cookies = HTTP.headers({ "Cookie" => cookie })
    
    response = client_with_cookies.get("https://httpbin.org/cookies")
    assert_equal(200, response.status)
    
    data = JSON.parse(response.body)
    assert_equal("value1", data["cookies"]["cookie1"])
    assert_equal("value2", data["cookies"]["cookie2"])
  end
  
  def test_random_user_agent
    # Make multiple requests and verify different user agents are used
    agents = []
    5.times do
      client = HTTP::Client.new
      response = client.get("https://tls.peet.ws/api/all")
      assert_equal(200, response.status)
      
      body = JSON.parse(response.body)
      # Get headers and extract user agent
      headers = get_headers_from_response(body)
      user_agent = headers.find { |h| h.start_with?("user-agent:") }
      
      # Fallback to the top-level user_agent if not found in headers
      user_agent = user_agent ? user_agent.sub("user-agent: ", "") : body["user_agent"]
      agents << user_agent
    end
    
    # Check that we got at least 2 different user agents (should be random)
    assert agents.uniq.size > 1, "Expected different random user agents, but got: #{agents.uniq}"
  end
  
  def test_desktop_client
    # Test the desktop client
    client = HTTP.desktop
    response = client.get("https://tls.peet.ws/api/all")
    assert_equal(200, response.status)
    
    body = JSON.parse(response.body)
    # Get headers and extract user agent
    headers = get_headers_from_response(body)
    user_agent = headers.find { |h| h.start_with?("user-agent:") }
    
    # Fallback to the top-level user_agent if not found in headers
    user_agent = user_agent ? user_agent.sub("user-agent: ", "") : body["user_agent"]
    
    # Verify it's a desktop user agent (doesn't have "Mobile" in it)
    refute_match(/Mobile/i, user_agent) if user_agent
  end
  
  def test_mobile_client
    # Test the mobile client
    client = HTTP.mobile
    response = client.get("https://tls.peet.ws/api/all")
    assert_equal(200, response.status)
    
    body = JSON.parse(response.body)
    # Get headers and extract user agent
    headers = get_headers_from_response(body)
    user_agent = headers.find { |h| h.start_with?("user-agent:") }
    
    # Fallback to the top-level user_agent if not found in headers
    user_agent = user_agent ? user_agent.sub("user-agent: ", "") : body["user_agent"]
    
    # Verify it's a mobile user agent - check for common mobile identifiers
    mobile_indicators = [/Mobile/i, /iPhone/i, /iPad/i, /iOS/i, /Android/i]
    is_mobile = mobile_indicators.any? { |pattern| user_agent =~ pattern }
    
    assert is_mobile, "Expected a mobile user agent, but got: #{user_agent}"
  end

  def test_headers
    response = HTTP
      .headers(accept: "application/json", user_agent: "Test Client")
      .get("https://tls.peet.ws/api/all")
    
    assert_equal(200, response.status)
    body = JSON.parse(response.body)
    
    # Get headers and check for accept header
    headers = get_headers_from_response(body)
    accept_header = headers.find { |h| h.start_with?("accept:") }
    
    # Verify the header contains application/json
    assert_match(/application\/json/, accept_header || "") if accept_header
  end

  def test_post_request
    response = HTTP.post(
      "https://httpbin.org/post",
      body: "test body"
    )
    
    assert_equal(200, response.status)
    body = JSON.parse(response.body)
    assert_equal("test body", body["data"])
  end

  def test_post_json
    response = HTTP
      .headers(content_type: "application/json")
      .post(
        "https://httpbin.org/post",
        body: JSON.generate({ name: "test", value: 123 })
      )
    
    assert_equal(200, response.status)
    body = JSON.parse(response.body)
    assert_equal({ "name" => "test", "value" => 123 }, JSON.parse(body["data"]))
  end

  def test_put_request
    response = HTTP.put(
      "https://httpbin.org/put",
      body: "updated content"
    )
    
    assert_equal(200, response.status)
    body = JSON.parse(response.body)
    assert_equal("updated content", body["data"])
  end

  def test_delete_request
    response = HTTP.delete("https://httpbin.org/delete")
    assert_equal(200, response.status)
  end

  def test_head_request
    response = HTTP.head("https://httpbin.org/get")
    assert_equal(200, response.status)
    assert_empty(response.body)
  end

  def test_patch_request
    response = HTTP.patch(
      "https://httpbin.org/patch",
      body: "patched content"
    )
    
    assert_equal(200, response.status)
    body = JSON.parse(response.body)
    assert_equal("patched content", body["data"])
  end

  def test_follow_redirects
    response = HTTP
      .follow(true)
      .get("https://httpbin.org/redirect/1")
    
    assert_equal(200, response.status)
    assert_equal("https://httpbin.org/get", response.uri)
  end

  def test_no_follow_redirects
    response = HTTP
      .follow(false)
      .get("https://httpbin.org/redirect/1")
    
    assert_equal(302, response.status)
    assert_equal("https://httpbin.org/redirect/1", response.uri)
  end

  def test_response_methods
    response = HTTP.get("https://tls.peet.ws/api/all")
    
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
      .headers(accept: "application/json")
      .get("https://tls.peet.ws/api/all")
    
    assert_equal("application/json", response.content_type)
    
    # Test with a response that has charset
    charset_response = HTTP.get("https://httpbin.org/html")
    assert_kind_of(String, charset_response.content_type)
    assert_includes(charset_response.content_type.to_s.downcase, "text/html")
    
    # Charset might be present depending on the server response
    if charset_response.charset
      assert_kind_of(String, charset_response.charset)
    end
  end

  def test_bing_search_results
    # Create a client with a common browser user agent to avoid being blocked
    client = HTTP::Client.new
    
    # Fetch Bing search results for "Coffee"
    response = client
      .headers(accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8")
      .get("https://www.bing.com/search?form=QBRE&q=Coffee&lq=0&rdr=1")
    
    assert_equal(200, response.status)
    
    # Convert response body to lowercase for case-insensitive checks
    body_text = response.body.downcase
    
    # Check for common elements that should be on a search results page
    has_search_results = body_text.include?("results") || body_text.include?("search") || body_text.include?("programming language")
    has_links = body_text.include?("<a href=") || body_text.include?("href=")
    
    # Assert that we have basic search results structure
    assert has_search_results, "No search results found in response"
    assert has_links, "No links found in search results"
  end
  
  def test_tls_fingerprinting
    # Create multiple clients to test fingerprint randomization
    fingerprints = []
    
    3.times do
      client = HTTP::Client.new
      
      # Make request to TLS fingerprinting service
      response = client.get("https://tls.peet.ws/api/all")
      assert_equal(200, response.status)
      
      # Parse the JSON response
      data = JSON.parse(response.body)
      
      # Verify TLS data structure
      assert_kind_of(Hash, data["tls"])
      assert_kind_of(Array, data["tls"]["ciphers"])
      assert(data["tls"]["ciphers"].size > 0, "Expected TLS ciphers to be present")
      
      # Check for JA3 and JA4 fingerprints
      refute_nil(data["tls"]["ja3"], "JA3 fingerprint should be present")
      refute_nil(data["tls"]["ja3_hash"], "JA3 hash should be present")
      refute_nil(data["tls"]["ja4"], "JA4 fingerprint should be present")
      
      # Store fingerprints for comparison
      fingerprints << {
        ja3: data["tls"]["ja3_hash"],
        ja4: data["tls"]["ja4"]
      }
      
      # Verify TLS version is modern
      tls_version = data["tls"]["tls_version_negotiated"]
      assert(["771", "772"].include?(tls_version), 
        "Expected modern TLS version (TLS 1.2 or 1.3), got: #{tls_version}")
    end
    
    # Check for fingerprint randomization
    # Either JA3 or JA4 should have some variation across requests 
    ja3_fingerprints = fingerprints.map { |f| f[:ja3] }.uniq
    ja4_fingerprints = fingerprints.map { |f| f[:ja4] }.uniq
    
    assert(ja3_fingerprints.size > 1 || ja4_fingerprints.size > 1,
      "Expected fingerprint randomization, but got identical fingerprints across requests")
  end

  def test_timeout_chainable
    skip "requires httpbin.org access"
    response = Wreq::HTTP.timeout(30).get("https://httpbin.org/get")
    assert_equal 200, response.status
  end

  def test_timeout_with_headers
    skip "requires httpbin.org access"
    response = Wreq::HTTP.headers(accept: "application/json").timeout(10).get("https://httpbin.org/get")
    assert_equal 200, response.status
  end

  def test_via_with_host_and_port
    skip "requires proxy server"
    response = Wreq::HTTP.via("proxy.example.com", 8080).get("https://httpbin.org/get")
    assert_equal 200, response.status
  end

  def test_via_with_auth
    skip "requires proxy server with auth"
    response = Wreq::HTTP.via("proxy.example.com", 8080, "user", "pass").get("https://httpbin.org/get")
    assert_equal 200, response.status
  end

  def test_via_chainable
    skip "requires proxy server"
    response = Wreq::HTTP.via("proxy.example.com", 8080).headers(accept: "application/json").get("https://httpbin.org/get")
    assert_equal 200, response.status
  end

  def test_status_object_success
    skip "requires httpbin.org access"
    response = Wreq::HTTP.get("https://httpbin.org/get")
    assert_instance_of Wreq::HTTP::Status, response.status
    assert_equal 200, response.status.to_i
    assert_equal "200 OK", response.status.to_s
    assert_equal "OK", response.status.reason
    assert response.status.success?
    assert response.status.ok?
    refute response.status.redirect?
    refute response.status.client_error?
    refute response.status.server_error?
  end

  def test_status_equality
    skip "requires httpbin.org access"
    response = Wreq::HTTP.get("https://httpbin.org/get")
    assert_equal 200, response.status
    assert response.status == 200
  end

  def test_status_404
    skip "requires httpbin.org access"
    response = Wreq::HTTP.get("https://httpbin.org/status/404")
    assert_equal 404, response.status.to_i
    assert_equal "Not Found", response.status.reason
    assert response.status.client_error?
    refute response.status.success?
  end

  def test_status_redirect
    skip "requires httpbin.org access"
    response = Wreq::HTTP.follow(false).get("https://httpbin.org/redirect/1")
    assert response.status.redirect?
    refute response.status.success?
  end

  def test_code_backward_compat
    skip "requires httpbin.org access"
    response = Wreq::HTTP.get("https://httpbin.org/get")
    assert_equal 200, response.code
    assert_kind_of Integer, response.code
  end

  def test_cookies
    skip "requires httpbin.org access"
    response = Wreq::HTTP.cookies(session: "abc123", user: "test").get("https://httpbin.org/cookies")
    assert_equal 200, response.status.to_i
    assert response.body.include?("abc123")
  end

  def test_basic_auth
    skip "requires httpbin.org access"
    response = Wreq::HTTP.basic_auth(user: "user", pass: "passwd").get("https://httpbin.org/basic-auth/user/passwd")
    assert_equal 200, response.status.to_i
  end

  def test_auth_bearer
    skip "requires httpbin.org access"
    response = Wreq::HTTP.auth("Bearer test-token").get("https://httpbin.org/bearer")
    assert_equal 401, response.status.to_i
  end

  def test_accept_symbol
    skip "requires httpbin.org access"
    response = Wreq::HTTP.accept(:json).get("https://httpbin.org/get")
    assert_equal 200, response.status.to_i
  end

  def test_accept_string
    skip "requires httpbin.org access"
    response = Wreq::HTTP.accept("text/html").get("https://httpbin.org/html")
    assert_equal 200, response.status.to_i
  end

  def test_chainable_auth_methods
    skip "requires httpbin.org access"
    response = Wreq::HTTP
      .cookies(session: "test")
      .headers(x_custom: "value")
      .accept(:json)
      .get("https://httpbin.org/get")
    assert_equal 200, response.status.to_i
  end

  def test_parse_json
    skip "requires httpbin.org access"
    response = Wreq::HTTP.get("https://httpbin.org/get")
    parsed = response.parse
    assert_instance_of Hash, parsed
    assert parsed.key?("url")
  end

  def test_parse_non_json
    skip "requires httpbin.org access"
    response = Wreq::HTTP.get("https://httpbin.org/html")
    parsed = response.parse
    assert_instance_of String, parsed
    assert parsed.include?("<html>")
  end

  def test_flush
    skip "requires httpbin.org access"
    response = Wreq::HTTP.get("https://httpbin.org/get")
    flushed = response.flush
    assert_equal response, flushed
  end

  def test_body_backward_compat
    skip "requires httpbin.org access"
    response = Wreq::HTTP.get("https://httpbin.org/get")
    assert_instance_of String, response.body
    assert response.body.length > 0
  end

  def test_follow_default
    skip "requires httpbin.org access"
    response = Wreq::HTTP.follow.get("https://httpbin.org/redirect/1")
    assert_equal 200, response.status.to_i
  end

  def test_follow_with_max_hops
    skip "requires httpbin.org access"
    response = Wreq::HTTP.follow(max_hops: 5).get("https://httpbin.org/redirect/3")
    assert_equal 200, response.status.to_i
  end

  def test_follow_true_backward_compat
    skip "requires httpbin.org access"
    response = Wreq::HTTP.follow(true).get("https://httpbin.org/redirect/1")
    assert_equal 200, response.status.to_i
  end

  def test_follow_false_backward_compat
    skip "requires httpbin.org access"
    response = Wreq::HTTP.follow(false).get("https://httpbin.org/redirect/1")
    assert response.status.redirect?
  end
end 
