# frozen_string_literal: true

require 'minitest/autorun'
require_relative '../lib/wreq_rb'
require 'json'

class ContentLengthTest < Minitest::Test
  HTTP = Wreq::HTTP

  def test_gzip_compressed_content_length
    response = HTTP.get('https://httpbin.org/gzip')
    assert_equal 200, response.status

    # Compressed wire size should be less than decompressed body size
    assert response.content_length < response.body.bytesize,
           "content_length (#{response.content_length}) should be < body.bytesize (#{response.body.bytesize})"

    # Body should be valid decompressed JSON
    data = JSON.parse(response.body)
    assert_equal true, data['gzipped']
  end

  def test_brotli_compressed_content_length
    response = HTTP.get('https://httpbin.org/brotli')
    assert_equal 200, response.status

    # Compressed wire size should be less than decompressed body size
    assert response.content_length < response.body.bytesize,
           "content_length (#{response.content_length}) should be < body.bytesize (#{response.body.bytesize})"

    # Body should be valid decompressed JSON
    data = JSON.parse(response.body)
    assert_equal true, data['brotli']
  end
  def test_uncompressed_content_length
    response = HTTP.get('https://httpbin.org/robots.txt')
    assert_equal 200, response.status

    # Uncompressed: wire size == body size
    assert_equal response.content_length, response.body.bytesize,
                 "content_length (#{response.content_length}) should equal body.bytesize (#{response.body.bytesize}) for uncompressed response"
  end

  def test_empty_body_head_request
    response = HTTP.head('https://httpbin.org/get')
    assert_equal 200, response.status

    # HEAD requests have no body - content_length should be 0
    # (response.body will be empty string, content_length tracks wire size)
    assert_equal 0, response.content_length,
                 'HEAD request should have content_length = 0'
  end
end
