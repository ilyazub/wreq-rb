require_relative "wreq_rb/version"

module Wreq
  module HTTP
    # Methods are defined by the native extension
  end
end

# Tries to require the extension for the given Ruby version first
begin
  RUBY_VERSION =~ /(\d+\.\d+)/
  require "wreq/#{Regexp.last_match(1)}/wreq_rb"
rescue LoadError
  require "wreq/wreq_rb"
end 
