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
