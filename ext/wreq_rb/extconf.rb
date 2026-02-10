require "mkmf"
require "rb_sys/mkmf"

create_rust_makefile("wreq_rb/wreq_rb") do |ext|
  ext.extra_cargo_args += ["--crate-type", "cdylib"]
  ext.extra_cargo_args += ["--package", "wreq-rb"]
end 
