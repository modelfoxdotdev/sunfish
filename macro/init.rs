use quote::{format_ident, quote};
use std::path::{Path, PathBuf};

pub fn init(_input: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
	let package_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
	let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
	let output_path = out_dir.join("output");
	let output_path_string = output_path.display().to_string();
	let routes_path = package_path.join("routes");
	let server_entries = server_entries(&routes_path);
	let routes_handler = routes_handler(&server_entries);
	let code = quote! {{
		sunfish::Sunfish {
			output: sunfish::include_dir!(#output_path_string),
			routes_handler: #routes_handler,
		}
	}};
	Ok(code)
}

#[derive(Debug)]
struct ServerEntry {
	package_name: String,
	path_with_placeholders: String,
}

fn server_entries(routes_path: &Path) -> Vec<ServerEntry> {
	let glob = routes_path
		.join("**")
		.join("server")
		.join("Cargo.toml")
		.display()
		.to_string();
	let mut entries = glob::glob(&glob)
		.unwrap()
		.filter_map(Result::ok)
		.map(|manifest_path| {
			let manifest = std::fs::read_to_string(&manifest_path).unwrap();
			let manifest: toml::Value = toml::from_str(&manifest).unwrap();
			let package_name = manifest
				.as_table()
				.unwrap()
				.get("package")
				.unwrap()
				.as_table()
				.unwrap()
				.get("name")
				.unwrap()
				.as_str()
				.unwrap()
				.to_owned();
			let path_with_placeholders = path_with_placeholders(routes_path, &manifest_path);
			ServerEntry {
				package_name,
				path_with_placeholders,
			}
		})
		.collect::<Vec<_>>();
	entries.sort_by(|a, b| a.path_with_placeholders.cmp(&b.path_with_placeholders));
	entries
}

fn path_with_placeholders(routes_path: &Path, manifest_path: &Path) -> String {
	let components = manifest_path
		.parent()
		.unwrap()
		.parent()
		.unwrap()
		.strip_prefix(&routes_path)
		.unwrap()
		.components()
		.map(|component| match component {
			std::path::Component::Prefix(_) => panic!(),
			std::path::Component::RootDir => panic!(),
			std::path::Component::CurDir => panic!(),
			std::path::Component::ParentDir => panic!(),
			std::path::Component::Normal(component) => component.to_str().unwrap(),
		});
	let mut path = String::new();
	for component in components {
		path.push('/');
		path.push_str(component);
	}
	if path.ends_with("/index") {
		path.truncate(path.len() - "index".len());
	}
	path
}

fn routes_handler(server_entries: &[ServerEntry]) -> proc_macro2::TokenStream {
	let match_arms = server_entries.iter().map(|server_entry| {
		let package_name = &server_entry.package_name;
		let server_package_name_ident = format_ident!("{}", server_entry.package_name);
		let path_components = server_entry
			.path_with_placeholders
			.split('/')
			.into_iter()
			.skip(1)
			.map(|path_component| match path_component {
				"_" => quote! { _ },
				"index" => quote! { "" },
				path_component => quote! { #path_component },
			})
			.collect::<Vec<_>>();
		quote! {
			#[cfg(feature = #package_name)]
			[#(#path_components),*] => {
				use futures::{Future, FutureExt, TryFutureExt};
				#server_package_name_ident::init().handle(request).map_ok(|response| Some(response)).boxed()
			}
		}
	});
	quote! {
		Box::new(|request| {
			let path = request.uri().path();
			let path_components: Vec<_> = path.split('/').skip(1).collect();
			match path_components.as_slice() {
				#(#match_arms)*
				_ => {
					use futures::{Future, FutureExt, TryFutureExt};
					async { Ok(None) }.boxed()
				}
			}
		})
	}
}
