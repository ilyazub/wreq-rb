require_relative "wreq_rb/version"
require "json"

module Wreq
  module HTTP
    class Response
      def parse
        ct = content_type
        if ct && ct.include?("application/json")
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
          if key.downcase == "set-cookie"
            # Simple extraction: name=value
            if value =~ /^([^=]+)=([^;]+)/
              cookies_hash[$1] = $2
            end
          end
        end
        cookies_hash
      end
    end

    class << self
      alias_method :through, :via
    end
  end
end

begin
  RUBY_VERSION =~ /(\d+\.\d+)/
  require "wreq/#{Regexp.last_match(1)}/wreq_rb"
rescue LoadError
  require "wreq/wreq_rb"
end 
