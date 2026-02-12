require "bundler/gem_tasks"
require "rake/testtask"
require "rb_sys/extensiontask"

GEMSPEC = Gem::Specification.load("wreq-rb.gemspec") || abort("Could not load wreq-rb.gemspec")

# Define the extension task using RbSys for proper cross-compilation support
RbSys::ExtensionTask.new("wreq-rb", GEMSPEC) do |ext|
  ext.lib_dir = "lib/wreq"
end

# Development tasks
task :fmt do
  sh 'cargo', 'fmt'
end

task :rust_test do
  sh "cargo test -- --test-threads=1 --nocapture"
end

# Run Ruby tests
Rake::TestTask.new(:ruby_test) do |t|
  t.libs << "test"
  t.libs << "lib"
  t.libs << File.expand_path("lib", __dir__)  # Add the lib directory to load path
  t.libs << File.expand_path("lib/wreq", __dir__)  # Add the native extension directory
  t.test_files = FileList["test/**/*_test.rb"]
  t.deps << :compile  # Make sure the native extension is built before running tests
end

task test: %i[rust_test ruby_test]

namespace :benchmark do
  desc "Run HTTP clients benchmark"
  task :http_clients_rb do
    puts "Running HTTP clients benchmark..."
    ruby 'benchmark/http_clients_benchmark.rb'
  end
end

desc "Run all benchmarks"
task :benchmark => ['benchmark:http_clients_rb']

task default: %i[compile test]
