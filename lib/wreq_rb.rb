require_relative 'wreq_rb/version'
require 'json'
require 'delegate'

# Load native extension first
begin
  RUBY_VERSION =~ /(\d+\.\d+)/
  require "wreq/#{Regexp.last_match(1)}/wreq_rb"
rescue LoadError
  require 'wreq/wreq_rb'
end

module Wreq
  module HTTP
    # Status object that delegates to Integer for == / kind_of? / assert_equal,
    # while still providing predicate helpers like ok?, redirect?, etc.
    class Status < Delegator
      REASONS = {
        100 => 'Continue', 101 => 'Switching Protocols', 102 => 'Processing',
        200 => 'OK', 201 => 'Created', 202 => 'Accepted',
        203 => 'Non-Authoritative Information', 204 => 'No Content',
        205 => 'Reset Content', 206 => 'Partial Content',
        207 => 'Multi-Status', 208 => 'Already Reported', 226 => 'IM Used',
        300 => 'Multiple Choices', 301 => 'Moved Permanently', 302 => 'Found',
        303 => 'See Other', 304 => 'Not Modified', 305 => 'Use Proxy',
        307 => 'Temporary Redirect', 308 => 'Permanent Redirect',
        400 => 'Bad Request', 401 => 'Unauthorized', 402 => 'Payment Required',
        403 => 'Forbidden', 404 => 'Not Found', 405 => 'Method Not Allowed',
        406 => 'Not Acceptable', 407 => 'Proxy Authentication Required',
        408 => 'Request Timeout', 409 => 'Conflict', 410 => 'Gone',
        411 => 'Length Required', 412 => 'Precondition Failed',
        413 => 'Payload Too Large', 414 => 'URI Too Long',
        415 => 'Unsupported Media Type', 416 => 'Range Not Satisfiable',
        417 => 'Expectation Failed', 418 => "I'm a teapot",
        421 => 'Misdirected Request', 422 => 'Unprocessable Entity',
        423 => 'Locked', 424 => 'Failed Dependency', 426 => 'Upgrade Required',
        428 => 'Precondition Required', 429 => 'Too Many Requests',
        431 => 'Request Header Fields Too Large',
        451 => 'Unavailable For Legal Reasons',
        500 => 'Internal Server Error', 501 => 'Not Implemented',
        502 => 'Bad Gateway', 503 => 'Service Unavailable',
        504 => 'Gateway Timeout', 505 => 'HTTP Version Not Supported',
        506 => 'Variant Also Negotiates', 507 => 'Insufficient Storage',
        508 => 'Loop Detected', 510 => 'Not Extended',
        511 => 'Network Authentication Required'
      }.freeze

      def initialize(code)
        @code = code.to_i
        super(@code)
      end

      def __getobj__
        @code
      end

      def __setobj__(obj)
        @code = obj
      end

      def to_s
        "#{@code} #{reason}"
      end

      def inspect
        "#<Wreq::HTTP::Status #{self}>"
      end

      def reason
        REASONS.fetch(@code, 'Unknown Status')
      end

      def to_i
        @code
      end

      def to_int
        @code
      end

      def is_a?(klass)
        klass == Integer || klass == Numeric || super
      end
      alias kind_of? is_a?

      def informational?
        @code >= 100 && @code < 200
      end

      def success?
        @code >= 200 && @code < 300
      end

      def ok?
        @code == 200
      end

      def redirect?
        @code >= 300 && @code < 400
      end

      def client_error?
        @code >= 400 && @code < 500
      end

      def server_error?
        @code >= 500 && @code < 600
      end
    end

    class Response
      alias raw_status status
      def status
        Status.new(raw_status)
      end

      def parse
        ct = content_type
        if ct&.include?('application/json')
          JSON.parse(body)
        else
          body
        end
      end

      def flush
        self
      end

      def cookies
        return {} unless headers

        cookies_hash = {}
        headers.each do |key, value|
          next unless key.downcase == 'set-cookie'

          # Simple extraction: name=value
          cookies_hash[::Regexp.last_match(1)] = ::Regexp.last_match(2) if value =~ /^([^=]+)=([^;]+)/
        end
        cookies_hash
      end
    end

    class << self
      alias through via

      # Override module-level persistent to add block support (yield + ensure close).
      # The Rust native extension defines a basic persistent that just returns a client.
      alias raw_persistent persistent
      def persistent(host, options = {})
        client = Client.new
        client = client.persistent(host, options)

        return client unless block_given?

        begin
          yield client
        ensure
          client.close
        end
      end
    end
  end
end
