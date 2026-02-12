require "bundler/gem_tasks"
require "rake/testtask"
require "rake/extensiontask"

GEMSPEC = Gem::Specification.load("wreq-rb.gemspec") || abort("Could not load wreq-rb.gemspec")

# Define supported platforms (focusing on Linux targets for cross-rs)
SUPPORTED_PLATFORMS = [
  "x86_64-linux",
  "arm64-linux"
]

# Helper to check if Docker/Podman is available
def container_runtime_available?
  system("which docker > /dev/null 2>&1") || system("which podman > /dev/null 2>&1")
end

# Helper to build for a specific platform
def build_for_platform(platform)
  puts "Building for platform: #{platform}"
  sh 'bundle', 'exec', 'rb-sys-dock', '--platform', platform, '--build'
end

# Define the extension task
Rake::ExtensionTask.new("wreq_rb", GEMSPEC) do |ext|
  ext.lib_dir = "lib/wreq"
  ext.ext_dir = "ext/wreq_rb"
  ext.cross_compile = true
  ext.cross_platform = SUPPORTED_PLATFORMS
end

# Build the native extension for the current platform
desc "Build the native extension for the current platform"
task :compile do
  sh "bundle"
  sh "bundle exec rake build"
end

# Build the gem for the current platform
desc "Build the gem for the current platform"
task :gem => :compile

desc "Cross-compile using cross-rs"
task :cross_compile do
  unless ENV['SKIP_CROSS_COMPILE']
    unless system("which cross > /dev/null")
      abort "Error: cross-rs not installed. Run: cargo install cross"
    end

    targets = %w[
      x86_64-unknown-linux-musl
      aarch64-unknown-linux-musl
    ]

    targets.each do |target|
      puts "\n\e[33mBuilding for #{target} using cross-rs\e[0m"
      sh "cross build --release --target #{target}"
    end

    # Verify artifacts in cross's target directory
    sh %(find target/cross -name '*.so')
  end
end

# Cross-compile and build native gems for all supported platforms
namespace "gem" do
  desc "Build native gems for all supported platforms"
  task "all-platforms" => [:clean] do
    require "rake_compiler_dock"
    
    if container_runtime_available?
      # Build using containers for each platform
      SUPPORTED_PLATFORMS.each { |platform| build_for_platform(platform) }
    else
      # Single cross-compile invocation for all platforms
      puts "Using local cross-compile for all platforms"
      Rake::Task[:cross_compile].invoke
    end
  end

  desc "Build native extension for a specific platform (e.g., `rake 'gem:native[x86_64-linux]'`)"
  task :native, [:platform] do |_t, args|
    platform = args[:platform]
    if platform.nil? || platform.empty?
      abort "Platform must be specified, e.g., rake 'gem:native[x86_64-linux]'"
    end
    
    unless container_runtime_available?
      abort "Docker or Podman is required for cross-compilation but not found. Please install one of them."
    end
    
    build_for_platform(platform)
  end
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

task default: %i[compile test] do
  if ENV['CROSS_COMPILE']
    Rake::Task[:cross_compile].invoke
  end
end
