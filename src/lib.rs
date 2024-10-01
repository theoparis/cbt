// Rust -> c bindings generator (generate extern "C" functions and a C header)

use convert_case::{Case, Casing};
use quote::quote;
use std::fs;
use std::path::Path;
use syn::{
	Field, FnArg, Item, ItemFn, ItemMod, ItemStruct, PatType, ReturnType, Type,
	TypeReference,
};

struct Argument {
	pub name: String,
	pub ty: Type,
	pub c_ty: String,
	pub extern_c_ty: String,
}

// Recursive function to generate both the C API and Rust wrappers
pub fn generate_c_api_and_rust_exports(
	syntax_tree: &[Item],
	base_path: &Path,
	parent_public: bool,
	crate_name: &str,
	mod_name: &str,
) -> (String, String) {
	let crate_name = crate_name.to_case(Case::Snake);
	let mod_name = mod_name.to_case(Case::Snake);
	let mut c_bindings = Vec::new();
	let mut rust_exports = Vec::new();

	// Iterate over the items in the syntax tree
	for item in syntax_tree {
		match item {
			Item::Fn(func) => {
				if is_public_function(func, parent_public) {
					if let Some((c_binding, rust_wrapper)) =
						generate_c_binding_and_rust_wrapper(func, &mod_name)
					{
						c_bindings.push(c_binding);
						rust_exports.push(rust_wrapper);
					}
				}
			}
			Item::Mod(module) => {
				if is_public_mod(module) {
					if let Some((_, items)) = &module.content {
						// Inline module: Recurse into the module to generate bindings for items within it
						let (module_c_bindings, module_rust_exports) =
							generate_c_api_and_rust_exports(
								items,
								base_path,
								true,
								&crate_name,
								&mod_name,
							);
						c_bindings.push(module_c_bindings);
						rust_exports.push(module_rust_exports);
					} else {
						// External module: Read the corresponding file and recurse
						if let Some((module_c_bindings, module_rust_exports)) =
							process_external_mod_and_generate_rust(
								module,
								base_path,
								&crate_name,
								&mod_name,
							) {
							c_bindings.push(module_c_bindings);
							rust_exports.push(module_rust_exports);
						}
					}
				}
			}
			Item::Struct(struct_item) => {
				if is_public_struct(struct_item, parent_public) {
					if let Some((c_struct_binding, rust_struct_wrapper)) =
						generate_c_struct_binding_and_rust_wrapper(
							struct_item,
							&crate_name,
						) {
						c_bindings.push(c_struct_binding);
						rust_exports.push(rust_struct_wrapper);
					}
				}
			}
			_ => {}
		}
	}

	let rust_bindings =
		format!("extern crate alloc;\n{}", rust_exports.join("\n"));
	let c_bindings = c_bindings.join("\n");

	(c_bindings, rust_bindings)
}

// Check if the function is public, accounting for module-level visibility
fn is_public_function(func: &ItemFn, parent_public: bool) -> bool {
	match &func.vis {
		syn::Visibility::Public(_) => true,
		_ => parent_public, // Function is considered public if it's inside a public module
	}
}

// Check if the struct is public, accounting for module-level visibility
fn is_public_struct(struct_item: &ItemStruct, parent_public: bool) -> bool {
	match &struct_item.vis {
		syn::Visibility::Public(_) => true,
		_ => parent_public, // Struct is considered public if it's inside a public module
	}
}

// Check if the module is public
fn is_public_mod(module: &ItemMod) -> bool {
	matches!(&module.vis, syn::Visibility::Public(_))
}

// Process an external module by reading its corresponding file and generating both C bindings and Rust wrappers
fn process_external_mod_and_generate_rust(
	module: &ItemMod,
	base_path: &Path,
	crate_name: &str,
	mod_name: &str,
) -> Option<(String, String)> {
	let module_name = module.ident.to_string();

	// Try both 'mod.rs' and '<module>.rs' patterns
	let mod_file_path = base_path.join(format!("{}.rs", module_name));
	let mod_folder_path = base_path.join(format!("{}/mod.rs", module_name));

	let mod_path = if mod_file_path.exists() {
		mod_file_path
	} else if mod_folder_path.exists() {
		mod_folder_path
	} else {
		return None; // If neither path exists, return None
	};

	// Read the module file content
	let mod_source =
		fs::read_to_string(&mod_path).expect("Unable to read module file");

	// Parse the module file content
	let mod_syntax_tree =
		syn::parse_file(&mod_source).expect("Unable to parse module file");

	// Recursively generate C API and Rust wrappers for the external module
	Some(generate_c_api_and_rust_exports(
		&mod_syntax_tree.items,
		mod_path.parent().unwrap(),
		true,
		crate_name,
		mod_name,
	))
}

// Generate both the C binding and Rust extern "C" wrapper for a Rust function
fn generate_c_binding_and_rust_wrapper(
	func: &ItemFn,
	module_name: &str,
) -> Option<(String, String)> {
	let func_name = &func.sig.ident;
	let func_name_c = format!("{}", func_name);

	// Generate function arguments
	let mut args = Vec::new();

	for input in &func.sig.inputs {
		if let FnArg::Typed(PatType { pat, ty, .. }) = input {
			let arg_name = quote! { #pat }.to_string();

			args.push(Argument {
				name: arg_name,
				ty: *ty.clone(),
				c_ty: rust_type_to_c(ty),
				extern_c_ty: rust_type_to_rust_extern_c(ty),
			})
		}
	}

	// Generate return type
	let ret_type = match &func.sig.output {
		ReturnType::Default => "void".to_string(),
		ReturnType::Type(_, ty) => rust_type_to_c(ty),
	};
	let rust_ret_type = match &func.sig.output {
		ReturnType::Default => "()".to_string(),
		ReturnType::Type(_, ty) => rust_type_to_rust_extern_c(ty),
	};

	// Generate the `extern "C"` function signature
	let c_binding = format!(
		"{} {}({});",
		ret_type,
		func_name_c,
		args.iter()
			.map(|arg| arg.c_ty.clone())
			.chain(std::iter::once("void*".to_string()))
			.collect::<Vec<_>>()
			.join(", "),
	);

	// Generate the Rust wrapper
	let mut rust_wrapper = format!(
		r#"
#[no_mangle]
pub extern "C" fn {func_name_c}({c_args}) -> {ret_type} {{
    use {module_name}::{func_name};
"#,
		func_name_c = func_name_c,
		c_args = args
			.iter()
			.map(|arg| format!("{}: {}", arg.name, arg.extern_c_ty))
			.collect::<Vec<_>>()
			.join(", "),
		ret_type = rust_ret_type,
		func_name = func_name,
	);

	for arg in &args {
		let arg_name = &arg.name;
		let rust_arg = &arg.ty;

		match rust_arg {
			Type::Reference(TypeReference { elem, .. }) => {
				if let Type::Path(p) = &**elem {
					if let Some(segment) = p.path.segments.first() {
						if segment.ident == "str" {
							rust_wrapper.push_str(&format!(
                            "    let {arg_name} = unsafe {{ std::ffi::CStr::from_ptr({arg_name}).to_str().unwrap() }};\n",
                            arg_name = arg_name,
                        ));
						}
					}
				} else {
					rust_wrapper.push_str(&format!(
                        "    let {arg_name} = unsafe {{ std::mem::transmute::<{extern_c_ty}, _>({arg_name}) }};\n",
                        arg_name = arg_name,
                        extern_c_ty = arg.extern_c_ty,
                    ));
				}
			}
			Type::Path(p) => match p.path.segments.first() {
				Some(segment) if segment.ident == "String" => {
					rust_wrapper.push_str(&format!(
                        "    let {arg_name} = unsafe {{ std::ffi::CStr::from_ptr({arg_name}).to_string_lossy().into_owned() }};\n",
                        arg_name = arg_name,
                    ));
				}
				_ => {
					rust_wrapper.push_str(&format!(
                        "    let {arg_name} = unsafe {{ std::mem::transmute::<{extern_c_ty}, _>({arg_name}) }};\n",
                        arg_name = arg_name,
                        extern_c_ty = arg.extern_c_ty,
                    ));
				}
			},
			_ => {
				rust_wrapper.push_str(&format!(
                    "    let {arg_name} = unsafe {{ std::mem::transmute::<{extern_c_ty}, _>({arg_name}) }};\n",
                    arg_name = arg_name,
                    extern_c_ty = arg.extern_c_ty,
                ));
			}
		}
	}

	rust_wrapper.push_str(&format!(
		"    let result = {func_name}({args});\n",
		func_name = func_name,
		args = args
			.iter()
			.map(|arg| arg.name.clone())
			.collect::<Vec<_>>()
			.join(", "),
	));

	if rust_ret_type != "void" {
		if rust_ret_type == "*mut core::ffi::c_char" {
			rust_wrapper.push_str("    let result = std::ffi::CString::new(result).unwrap().into_raw();\n");
			rust_wrapper.push_str("    result\n");
		} else {
			rust_wrapper.push_str(&format!(
			"    unsafe {{ std::mem::transmute::<_, {ret_type}>(result) }}\n",
			ret_type = rust_ret_type,
    		));
		}
	}

	rust_wrapper.push_str("}\n");

	Some((c_binding, rust_wrapper))
}

// Generate both the C struct and the Rust extern "C" struct handling functions (constructor, destructor, etc.)
fn generate_c_struct_binding_and_rust_wrapper(
	struct_item: &ItemStruct,
	mod_name: &str,
) -> Option<(String, String)> {
	let struct_name = &struct_item.ident;
	let struct_name_c = struct_name.to_string();
	let struct_name_c = struct_name_c.to_case(Case::Snake);

	// Generate C struct definition
	let mut fields = Vec::new();
	for field in &struct_item.fields {
		if let Some(field_binding) = generate_c_struct_field(field) {
			fields.push(field_binding);
		}
	}

	let c_struct = format!(
		"typedef struct {} {{\n{}\n}} {};",
		struct_name_c,
		fields.join("\n"),
		struct_name_c
	);

	// Generate constructor, destructor, and accessor functions in C
	let constructor = format!("{}* {}_new();", struct_name_c, struct_name_c);
	let destructor =
		format!("void {}_free({}* obj);", struct_name_c, struct_name_c);

	let bindings = format!("{}\n{}\n{}", c_struct, constructor, destructor);

	// Generate the Rust implementation for the constructor and destructor
	let rust_struct_wrapper = format!(
		r#"
use alloc::boxed::Box;
use {mod_name}::{struct_name};

#[no_mangle]
pub extern "C" fn {struct_name_c}_new() -> *mut {struct_name} {{
    Box::into_raw(Box::new({struct_name}::default()))
}}

#[no_mangle]
pub extern "C" fn {struct_name_c}_free(obj: *mut {struct_name}) {{
    if !obj.is_null() {{
        unsafe {{
            let _ = Box::from_raw(obj);
        }}
    }}
}}
"#,
		struct_name_c = struct_name_c,
		struct_name = struct_name
	);

	Some((bindings, rust_struct_wrapper))
}

// Generate C field for a Rust struct field
fn generate_c_struct_field(field: &Field) -> Option<String> {
	let field_name = field.ident.as_ref()?.to_string();
	let c_type = rust_type_to_c(&field.ty);
	Some(format!("    {} {};", c_type, field_name))
}

// Map Rust types to C types, including std types
fn rust_type_to_c(ty: &Type) -> String {
	match ty {
		Type::Path(type_path) => {
			let ident = &type_path.path.segments.first().unwrap().ident;
			match ident.to_string().as_str() {
				// Handle basic Rust types
				"i32" => "int".to_string(),
				"f64" => "double".to_string(),
				"u32" => "unsigned int".to_string(),
				"bool" => "bool".to_string(),

				// Handle std types
				"String" => "char*".to_string(), // For String, we use C's const char*
				"Vec" => format!("{}*", "void"), // Vec<T> will need special handling
				"Option" => format!("{}*", "void"), // Option<T> is mapped to pointers

				_ => format!("{}*", ident), // Handle custom structs as pointers
			}
		}
		_ => "void*".to_string(), // default for non-path types
	}
}

fn rust_type_to_rust_extern_c(ty: &Type) -> String {
	match ty {
		Type::Path(type_path) => {
			let ident = &type_path.path.segments.first().unwrap().ident;
			match ident.to_string().as_str() {
				// Handle basic Rust types
				"i32" => "i32".to_string(),
				"f64" => "f64".to_string(),
				"u32" => "u32".to_string(),
				"bool" => "bool".to_string(),

				// Handle std types
				"String" => "*mut core::ffi::c_char".to_string(), // For String, we use C's const char*
				"Vec" => "*mut cor:::ffi::c_void".to_string(), // Vec<T> will need special handling
				"Option" => "*mut core::ffi::c_void".to_string(), // Option<T> is mapped to pointers
				_ => format!("{}", ident), // Handle custom structs as pointers
			}
		}
		_ => "void".to_string(), // default for non-path types
	}
}
