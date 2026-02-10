require_relative 'wreq_rb/version'
require 'json'

# Load native extension first
begin
  RUBY_VERSION =~ /(\d+\.\d+)/
  require "wreq/#{Regexp.last_match(1)}/wreq_rb"
rescue LoadError
  require 'wreq/wreq_rb'
end

module Wreq
  module HTTP
    class Response
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

      def persistent(host, options = {})
        client = new
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
