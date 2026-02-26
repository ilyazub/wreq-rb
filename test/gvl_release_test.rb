# frozen_string_literal: true

require 'minitest/autorun'
require_relative '../lib/wreq_rb'

class GVLReleaseTest < Minitest::Test
  HTTP = Wreq::HTTP

  def test_gvl_released_during_http_request
    counter = 0
    thread = Thread.new { loop { counter += 1 } }

    # HTTP request with 1-second delay
    response = HTTP.get('https://httpbin.org/delay/1')
    thread.kill

    # If GVL released, counter should be high (millions)
    # Use threshold > 1M to avoid flakiness
    assert counter > 1_000_000,
           "Counter (#{counter}) should be > 1M if GVL released during HTTP I/O. " \
           'Low counter means GVL was held, blocking concurrent threads.'
    assert_equal 200, response.status
  end

  # NOTE: WREQ_RB_NO_GVL_RELEASE env var bypass test is skipped because:
  # - Rust's std::env::var() reads process environment at startup
  # - Ruby's ENV['X'] = 'Y' does NOT propagate to Rust's std::env
  # - Would require subprocess test: system("WREQ_RB_NO_GVL_RELEASE=1 ruby ...")
  # - Manual verification from shell works: WREQ_RB_NO_GVL_RELEASE=1 bundle exec ruby -e "..."
end
