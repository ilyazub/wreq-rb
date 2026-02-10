#!/usr/bin/env ruby

require "benchmark/ips"
require "curb"
require "typhoeus"
require "httpx"
require "http"
require "wreq_rb"

URL = "https://serpapi.com/robots.txt"

Benchmark.ips do |x|
  x.config(warmup: 5, time: 20)

  x.report("curb") do
    Curl.get(URL).status
  end

  x.report("http.rb") do
    HTTP.get(URL).status
  end

  x.report("wreq-rb") do
    Wreq::HTTP.get(URL).code
  end
  
  x.report("typhoeus") do
    Typhoeus.get(URL).code
  end
  
  x.report("httpx") do
    HTTPX.get(URL).status
  end

  x.compare!
end
