use std::{
	fs::{self, File},
	io::Write,
	path::PathBuf,
};

use cbt::generate_c_api_and_rust_exports;
use clap::Parser;

#[derive(Parser)]
struct Args {
	#[clap(short, long)]
	input_file: PathBuf,

	#[clap(short, long)]
	output_dir: String,

	#[clap(short, long)]
	crate_name: String,
}

fn main() {
	let args = Args::parse();

	let output_dir = std::path::Path::new(&args.output_dir);
	if !output_dir.exists() {
		std::fs::create_dir_all(output_dir.join("src")).unwrap();
	}

	let mut output_cargo_toml =
		File::create(output_dir.join("Cargo.toml")).unwrap();
	let mut output_lib_rs =
		File::create(output_dir.join("src/lib.rs")).unwrap();
	let mut output_c_header =
		File::create(output_dir.join("src/bindings.h")).unwrap();

	output_cargo_toml
		.write_all(
			format!(
				r#"
[package]
name = "{crate_name}_c_api"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "staticlib"]

[dependencies]
{crate_name} = {{ path = "../" }}
			"#,
				crate_name = args.crate_name
			)
			.as_bytes(),
		)
		.unwrap();

	let input_file = fs::read_to_string(&args.input_file).unwrap();
	let syntax_tree = syn::parse_file(&input_file).unwrap();
	let (c_api, rust_exports) = generate_c_api_and_rust_exports(
		&syntax_tree.items,
		&args.input_file,
		false,
		&args.crate_name,
		&args.crate_name,
	);

	output_lib_rs.write_all(rust_exports.as_bytes()).unwrap();
	output_c_header.write_all(c_api.as_bytes()).unwrap();

	std::process::Command::new("rustfmt")
		.arg(output_dir.join("src/lib.rs"))
		.output()
		.unwrap();
	std::process::Command::new("clang-format")
		.arg("-i")
		.arg(output_dir.join("src/bindings.h"))
		.output()
		.unwrap();

	println!("Generated C bindings in {}", args.output_dir);
}
