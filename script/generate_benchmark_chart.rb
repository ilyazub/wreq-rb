#!/usr/bin/env ruby
# frozen_string_literal: true

require 'csv'
require 'date'
require 'fileutils'
require 'graphviz'

class BenchmarkChartGenerator
  def initialize(input_file, output_dir)
    @input_file = input_file
    @output_dir = output_dir
    validate_inputs
    FileUtils.mkdir_p(@output_dir)
  end

  def generate
    data = load_data
    generate_time_chart(data)
    generate_rps_chart(data)
  end

  private

  def validate_inputs
    unless File.exist?(@input_file)
      puts "Error: File #{@input_file} does not exist."
      exit 1
    end
  end

  def load_data
    data = CSV.read(@input_file, headers: true)
    # Limit to last 10 entries but keep at least 5 for meaningful comparison
    data.size > 10 ? data.last(10) : data
  end

  def extract_metrics(data)
    {
      dates: data.map { |row| row['date'] },
      commits: data.map { |row| row['commit'][0..7] }, # Short commit hash
      curb_time: data.map { |row| row['curb_time'].to_f },
      http_time: data.map { |row| row['http_time'].to_f },
      wreq_time: data.map { |row| row['wreq_time'].to_f },
      curb_rps: data.map { |row| row['curb_req_per_sec'].to_f },
      http_rps: data.map { |row| row['http_req_per_sec'].to_f },
      wreq_rps: data.map { |row| row['wreq_req_per_sec'].to_f }
    }
  end

  def generate_time_chart(data)
    metrics = extract_metrics(data)
    
    GraphViz.new(:G, type: :digraph) do |g|
      configure_graph(g, 'Request Time Comparison (seconds, lower is better)')
      
      prev_node = nil
      metrics[:commits].each_with_index do |commit, i|
        node = create_time_node(g, commit, metrics, i)
        connect_nodes(g, prev_node, node)
        prev_node = node
      end
      
      add_legend(g)
    end.output(png: "#{@output_dir}/time_chart.png", svg: "#{@output_dir}/time_chart.svg")
  end

  def create_time_node(graph, commit, metrics, index)
    label = "#{commit}\\n#{metrics[:dates][index]}"
    html_label = <<~HTML
      <TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="4">
        <TR><TD COLSPAN="2" ALIGN="CENTER">#{label}</TD></TR>
        <TR><TD ALIGN="RIGHT">Curb:</TD><TD BGCOLOR="#FF9999" WIDTH="#{(metrics[:curb_time][index] * 50).to_i}"> #{metrics[:curb_time][index].round(2)}s</TD></TR>
        <TR><TD ALIGN="RIGHT">HTTP.rb:</TD><TD BGCOLOR="#99CCFF" WIDTH="#{(metrics[:http_time][index] * 50).to_i}"> #{metrics[:http_time][index].round(2)}s</TD></TR>
        <TR><TD ALIGN="RIGHT">Wreq-rb:</TD><TD BGCOLOR="#99FF99" WIDTH="#{(metrics[:wreq_time][index] * 50).to_i}"> #{metrics[:wreq_time][index].round(2)}s</TD></TR>
      </TABLE>
    HTML
    
    graph.add_nodes("data#{index}", label: html_label)
  end

  def generate_rps_chart(data)
    metrics = extract_metrics(data)
    max_rps = [metrics[:curb_rps].max, metrics[:http_rps].max, metrics[:wreq_rps].max].max
    scale_factor = 200.0 / max_rps

    GraphViz.new(:G, type: :digraph) do |g|
      configure_graph(g, 'Requests Per Second Comparison (higher is better)')
      
      prev_node = nil
      metrics[:commits].each_with_index do |commit, i|
        node = create_rps_node(g, commit, metrics, i, scale_factor)
        connect_nodes(g, prev_node, node)
        prev_node = node
      end
      
      add_legend(g)
    end.output(png: "#{@output_dir}/rps_chart.png", svg: "#{@output_dir}/rps_chart.svg")
  end

  def create_rps_node(graph, commit, metrics, index, scale_factor)
    label = "#{commit}\\n#{metrics[:dates][index]}"
    html_label = <<~HTML
      <TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="4">
        <TR><TD COLSPAN="2" ALIGN="CENTER">#{label}</TD></TR>
        <TR><TD ALIGN="RIGHT">Curb:</TD><TD BGCOLOR="#FF9999" WIDTH="#{(metrics[:curb_rps][index] * scale_factor).to_i}"> #{metrics[:curb_rps][index].round(2)}</TD></TR>
        <TR><TD ALIGN="RIGHT">HTTP.rb:</TD><TD BGCOLOR="#99CCFF" WIDTH="#{(metrics[:http_rps][index] * scale_factor).to_i}"> #{metrics[:http_rps][index].round(2)}</TD></TR>
        <TR><TD ALIGN="RIGHT">Wreq-rb:</TD><TD BGCOLOR="#99FF99" WIDTH="#{(metrics[:wreq_rps][index] * scale_factor).to_i}"> #{metrics[:wreq_rps][index].round(2)}</TD></TR>
      </TABLE>
    HTML
    
    graph.add_nodes("data#{index}", label: html_label)
  end

  def configure_graph(graph, title)
    graph[:rankdir] = 'LR'
    graph[:bgcolor] = 'transparent'
    graph.node[:shape] = 'none'
    graph.node[:fontname] = 'Arial'
    graph.edge[:fontname] = 'Arial'
    graph.edge[:style] = 'invis'
    graph[:ranksep] = '0.1'
    graph[:label] = title
    graph[:labelloc] = 't'
  end

  def connect_nodes(graph, prev_node, node)
    graph.add_edges(prev_node, node) if prev_node
  end

  def add_legend(graph)
    legend_html = <<~HTML
      <TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="4">
        <TR><TD COLSPAN="2" ALIGN="CENTER"><B>Legend</B></TD></TR>
        <TR><TD BGCOLOR="#FF9999" WIDTH="20"></TD><TD>Curb</TD></TR>
        <TR><TD BGCOLOR="#99CCFF" WIDTH="20"></TD><TD>HTTP.rb</TD></TR>
        <TR><TD BGCOLOR="#99FF99" WIDTH="20"></TD><TD>Wreq-rb</TD></TR>
      </TABLE>
    HTML
    graph.add_nodes("legend", label: legend_html)
  end
end

# Main execution
if __FILE__ == $PROGRAM_NAME
  input_file = ARGV[0] || 'benchmark-results/benchmark-history.csv'
  output_dir = ARGV[1] || 'benchmark-results'
  
  generator = BenchmarkChartGenerator.new(input_file, output_dir)
  generator.generate
  
  puts "Generated benchmark charts in #{output_dir}:"
  puts "- time_chart.{png,svg}"
  puts "- rps_chart.{png,svg}"
end
