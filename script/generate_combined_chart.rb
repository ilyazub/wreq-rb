#!/usr/bin/env ruby
# frozen_string_literal: true

require 'csv'
require 'date'
require 'fileutils'
require 'graphviz'

# Ensure output directory exists
output_dir = ARGV[0] || 'benchmark-results'
FileUtils.mkdir_p(output_dir)

# Find all benchmark history files
ruby_versions = ['2.7', '3.0', '3.1', '3.2', '3.3', '3.4']
history_files = ruby_versions.map do |ruby_version|
  file = "#{output_dir}/benchmark-history-#{ruby_version}.csv"
  [ruby_version, file] if File.exist?(file)
end.compact

if history_files.empty?
  puts "Error: No benchmark history files found."
  exit 1
end

puts "Found benchmark history files for Ruby versions: #{history_files.map(&:first).join(', ')}"

# Process benchmark data from each Ruby version
all_data = []

history_files.each do |ruby_version, file|
  begin
    # Read the CSV data as an array of rows
    rows = CSV.read(file, headers: true).map(&:to_h)
    
    # Get the last row (most recent benchmark)
    if rows.any?
      latest = rows.last
      
      all_data << {
        ruby_version: ruby_version,
        commit: latest["commit"][0..7], 
        date: latest["date"],
        curb_time: latest["curb_time"].to_f,
        http_time: latest["http_time"].to_f,
        wreq_time: latest["wreq_time"].to_f,
        curb_rps: latest["curb_req_per_sec"].to_f,
        http_rps: latest["http_req_per_sec"].to_f,
        wreq_rps: latest["wreq_req_per_sec"].to_f
      }
    end
  rescue => e
    puts "Error processing #{file}: #{e.message}"
  end
end

if all_data.empty?
  puts "Error: No benchmark data could be processed."
  exit 1
end

# Sort by Ruby version
all_data.sort_by! { |d| d[:ruby_version] }

# Generate a combined chart for request time (lower is better)
time_graph = GraphViz.new(:G, type: :digraph) do |g|
  g[:rankdir] = 'TB'  # Top to bottom layout
  g[:bgcolor] = 'transparent'
  g.node[:shape] = 'none'
  g.node[:fontname] = 'Arial'
  g[:label] = 'HTTP Client Performance by Ruby Version - Request Time (seconds, lower is better)'
  g[:labelloc] = 't'
  g[:fontsize] = '18'
  
  # Calculate the maximum time for scaling the bars
  max_time = all_data.map { |d| [d[:curb_time], d[:http_time], d[:wreq_time]].max }.max
  scale_factor = 400.0 / max_time  # Scale to fit within 400 pixels
  
  # Create a node for the table
  html_label = <<~HTML
    <TABLE BORDER="0" CELLBORDER="1" CELLSPACING="0" CELLPADDING="8">
      <TR>
        <TD BGCOLOR="#EEEEEE"><B>Ruby Version</B></TD>
        <TD BGCOLOR="#EEEEEE"><B>Curb</B></TD>
        <TD BGCOLOR="#EEEEEE"><B>HTTP.rb</B></TD>
        <TD BGCOLOR="#EEEEEE"><B>Wreq-rb</B></TD>
        <TD BGCOLOR="#EEEEEE"><B>Visual Comparison (seconds)</B></TD>
      </TR>
  HTML
  
  # Add rows for each Ruby version
  all_data.each do |data|
    html_label += <<~HTML
      <TR>
        <TD>#{data[:ruby_version]}</TD>
        <TD>#{data[:curb_time].round(2)}</TD>
        <TD>#{data[:http_time].round(2)}</TD>
        <TD>#{data[:wreq_time].round(2)}</TD>
        <TD>
          <TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="2">
            <TR>
              <TD ALIGN="RIGHT">Curb</TD>
              <TD BGCOLOR="#FF9999" WIDTH="#{(data[:curb_time] * scale_factor).to_i}"> </TD>
              <TD ALIGN="LEFT">#{data[:curb_time].round(2)}s</TD>
            </TR>
            <TR>
              <TD ALIGN="RIGHT">HTTP.rb</TD>
              <TD BGCOLOR="#99CCFF" WIDTH="#{(data[:http_time] * scale_factor).to_i}"> </TD>
              <TD ALIGN="LEFT">#{data[:http_time].round(2)}s</TD>
            </TR>
            <TR>
              <TD ALIGN="RIGHT">Wreq-rb</TD>
              <TD BGCOLOR="#99FF99" WIDTH="#{(data[:wreq_time] * scale_factor).to_i}"> </TD>
              <TD ALIGN="LEFT">#{data[:wreq_time].round(2)}s</TD>
            </TR>
          </TABLE>
        </TD>
      </TR>
    HTML
  end
  
  # Close the table
  html_label += '</TABLE>'
  
  # Add the table node
  g.add_nodes('time_table', label: html_label)
end

# Generate a combined chart for requests per second (higher is better)
rps_graph = GraphViz.new(:G, type: :digraph) do |g|
  g[:rankdir] = 'TB'  # Top to bottom layout
  g[:bgcolor] = 'transparent'
  g.node[:shape] = 'none'
  g.node[:fontname] = 'Arial'
  g[:label] = 'HTTP Client Performance by Ruby Version - Requests Per Second (higher is better)'
  g[:labelloc] = 't'
  g[:fontsize] = '18'
  
  # Calculate the maximum RPS for scaling the bars
  max_rps = all_data.map { |d| [d[:curb_rps], d[:http_rps], d[:wreq_rps]].max }.max
  scale_factor = 400.0 / max_rps  # Scale to fit within 400 pixels
  
  # Create a node for the table
  html_label = <<~HTML
    <TABLE BORDER="0" CELLBORDER="1" CELLSPACING="0" CELLPADDING="8">
      <TR>
        <TD BGCOLOR="#EEEEEE"><B>Ruby Version</B></TD>
        <TD BGCOLOR="#EEEEEE"><B>Curb</B></TD>
        <TD BGCOLOR="#EEEEEE"><B>HTTP.rb</B></TD>
        <TD BGCOLOR="#EEEEEE"><B>Wreq-rb</B></TD>
        <TD BGCOLOR="#EEEEEE"><B>Visual Comparison (req/s)</B></TD>
      </TR>
  HTML
  
  # Add rows for each Ruby version
  all_data.each do |data|
    html_label += <<~HTML
      <TR>
        <TD>#{data[:ruby_version]}</TD>
        <TD>#{data[:curb_rps].round(2)}</TD>
        <TD>#{data[:http_rps].round(2)}</TD>
        <TD>#{data[:wreq_rps].round(2)}</TD>
        <TD>
          <TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="2">
            <TR>
              <TD ALIGN="RIGHT">Curb</TD>
              <TD BGCOLOR="#FF9999" WIDTH="#{(data[:curb_rps] * scale_factor).to_i}"> </TD>
              <TD ALIGN="LEFT">#{data[:curb_rps].round(2)}</TD>
            </TR>
            <TR>
              <TD ALIGN="RIGHT">HTTP.rb</TD>
              <TD BGCOLOR="#99CCFF" WIDTH="#{(data[:http_rps] * scale_factor).to_i}"> </TD>
              <TD ALIGN="LEFT">#{data[:http_rps].round(2)}</TD>
            </TR>
            <TR>
              <TD ALIGN="RIGHT">Wreq-rb</TD>
              <TD BGCOLOR="#99FF99" WIDTH="#{(data[:wreq_rps] * scale_factor).to_i}"> </TD>
              <TD ALIGN="LEFT">#{data[:wreq_rps].round(2)}</TD>
            </TR>
          </TABLE>
        </TD>
      </TR>
    HTML
  end
  
  # Close the table
  html_label += '</TABLE>'
  
  # Add the table node
  g.add_nodes('rps_table', label: html_label)
end

# Save charts
begin
  time_graph.output(png: "#{output_dir}/combined_time_chart.png")
  time_graph.output(svg: "#{output_dir}/combined_time_chart.svg")
  rps_graph.output(png: "#{output_dir}/combined_rps_chart.png")
  rps_graph.output(svg: "#{output_dir}/combined_rps_chart.svg")
  
  puts "Generated combined benchmark charts:"
  puts "- #{output_dir}/combined_time_chart.png"
  puts "- #{output_dir}/combined_time_chart.svg"
  puts "- #{output_dir}/combined_rps_chart.png"
  puts "- #{output_dir}/combined_rps_chart.svg"
rescue StandardError => e
  puts "Error generating charts with GraphViz: #{e.message}"
  puts "Creating text-based output files instead..."
  
  # Create text-based versions as a fallback
  File.open("#{output_dir}/combined_time_chart.txt", 'w') do |f|
    f.puts "HTTP Client Performance by Ruby Version - Request Time (seconds, lower is better)"
    f.puts "-" * 100
    f.puts "Ruby Version | Curb     | HTTP.rb  | Wreq-rb"
    f.puts "-" * 100
    
    all_data.each do |data|
      f.puts sprintf("%-12s | %-8.2f | %-8.2f | %-8.2f", 
        data[:ruby_version], 
        data[:curb_time], 
        data[:http_time], 
        data[:wreq_time]
      )
    end
  end
  
  File.open("#{output_dir}/combined_rps_chart.txt", 'w') do |f|
    f.puts "HTTP Client Performance by Ruby Version - Requests Per Second (higher is better)"
    f.puts "-" * 100
    f.puts "Ruby Version | Curb     | HTTP.rb  | Wreq-rb"
    f.puts "-" * 100
    
    all_data.each do |data|
      f.puts sprintf("%-12s | %-8.2f | %-8.2f | %-8.2f", 
        data[:ruby_version], 
        data[:curb_rps], 
        data[:http_rps], 
        data[:wreq_rps]
      )
    end
  end
  
  puts "Generated text-based benchmark summaries:"
  puts "- #{output_dir}/combined_time_chart.txt"
  puts "- #{output_dir}/combined_rps_chart.txt"
end 
