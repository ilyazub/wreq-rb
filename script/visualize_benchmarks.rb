#!/usr/bin/env ruby
# frozen_string_literal: true

require 'csv'
require 'date'
require 'optparse'

options = {
  file: 'benchmark-history-2.7.csv',
  metric: 'requests_per_second', # or 'time'
  limit: 10
}

OptionParser.new do |opts|
  opts.banner = "Usage: #{$PROGRAM_NAME} [options]"

  opts.on('-f', '--file FILE', 'Benchmark history CSV file path') do |file|
    options[:file] = file
  end

  opts.on('-m', '--metric TYPE', %w[time requests_per_second], 'Metric type (time or requests_per_second)') do |metric|
    options[:metric] = metric
  end

  opts.on('-l', '--limit NUM', Integer, 'Limit to last N entries') do |limit|
    options[:limit] = limit
  end

  opts.on('-h', '--help', 'Show this message') do
    puts opts
    exit
  end
end.parse!

puts "Visualizing benchmark history from #{options[:file]}"
puts "Metric: #{options[:metric]}"
puts "--------------------------------------------"

begin
  data = CSV.read(options[:file], headers: true)
rescue Errno::ENOENT
  puts "Error: File #{options[:file]} does not exist."
  exit 1
end

# Limit to last N entries
data = data.last(options[:limit]) if options[:limit] > 0

# Extract data
dates = data.map { |row| row['date'] }
commits = data.map { |row| row['commit'][0..7] } # Short commit hash
ruby_versions = data.map { |row| row['ruby_version'] || 'unknown' }

if options[:metric] == 'time'
  curb_vals = data.map { |row| row['curb_time'].to_f }
  http_vals = data.map { |row| row['http_time'].to_f }
  wreq_vals = data.map { |row| row['wreq_time'].to_f }
  label = 'Time (seconds)'
else
  curb_vals = data.map { |row| row['curb_req_per_sec'].to_f }
  http_vals = data.map { |row| row['http_req_per_sec'].to_f }
  wreq_vals = data.map { |row| row['wreq_req_per_sec'].to_f }
  label = 'Requests per second'
end

# Find max value for scaling in ASCII chart
max_val = [curb_vals.max, http_vals.max, wreq_vals.max].max
chart_width = 60

# Display table
puts "\nBenchmark History (#{label})"
puts "-" * 90
puts "%-10s %-8s %-6s %-12s %-12s %-12s" % ['Date', 'Commit', 'Ruby', 'Curb', 'HTTP.rb', 'Wreq-rb']
puts "-" * 90

dates.each_with_index do |date, i|
  puts "%-10s %-8s %-6s %-12.2f %-12.2f %-12.2f" % [
    date, 
    commits[i], 
    ruby_versions[i],
    curb_vals[i], 
    http_vals[i], 
    wreq_vals[i]
  ]
end

# Simple ASCII chart
puts "\nTrend Visualization (#{label}):"
puts "-" * 90

puts "Curb:"
curb_vals.each_with_index do |val, i|
  bar_length = (val * chart_width / max_val).to_i
  puts "#{commits[i]} [Ruby #{ruby_versions[i]}]: #{'#' * bar_length} #{val.round(2)}"
end

puts "\nHTTP.rb:"
http_vals.each_with_index do |val, i|
  bar_length = (val * chart_width / max_val).to_i
  puts "#{commits[i]} [Ruby #{ruby_versions[i]}]: #{'#' * bar_length} #{val.round(2)}"
end

puts "\nWreq-rb:"
wreq_vals.each_with_index do |val, i|
  bar_length = (val * chart_width / max_val).to_i
  puts "#{commits[i]} [Ruby #{ruby_versions[i]}]: #{'#' * bar_length} #{val.round(2)}"
end

# Performance comparison
puts "\nPerformance Summary:"
puts "-" * 90

latest_idx = curb_vals.size - 1
latest_curb = curb_vals[latest_idx]
latest_http = http_vals[latest_idx]
latest_wreq = wreq_vals[latest_idx]
latest_ruby = ruby_versions[latest_idx]

if options[:metric] == 'time'
  puts "In the latest benchmark (Ruby #{latest_ruby}):"
  puts "- Wreq-rb is #{(latest_curb / latest_wreq).round(2)}x faster than Curb"
  puts "- Wreq-rb is #{(latest_http / latest_wreq).round(2)}x faster than HTTP.rb"
else
  puts "In the latest benchmark (Ruby #{latest_ruby}):"
  puts "- Wreq-rb handles #{(latest_wreq / latest_curb).round(2)}x more requests per second than Curb"
  puts "- Wreq-rb handles #{(latest_wreq / latest_http).round(2)}x more requests per second than HTTP.rb"
end 
