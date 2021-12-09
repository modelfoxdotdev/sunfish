use quote::{format_ident, quote};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn init(_input: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
	let workspace_path = std::env::current_dir().unwrap();
	let workspace_path_string = workspace_path.display().to_string();
	let package_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
	let package_path_string = package_path.display().to_string();
	let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
	let output_path = out_dir.join("output");
	let output_path_string = output_path.display().to_string();
	std::fs::create_dir_all(&output_path).unwrap();
	let routes_path = package_path.join("routes");
	let embedded_directory = embedded_directory(&output_path);
	let server_entries = server_entries(&routes_path);
	let routes_handler = routes_handler(&server_entries);
	let routes = routes(&server_entries);
	let code = quote! {{
		#[cfg(debug_assertions)]
		{
			sunfish::Sunfish::Debug(sunfish::DebugSunfish {
				workspace_path: std::path::PathBuf::from(#workspace_path_string),
				package_path: std::path::PathBuf::from(#package_path_string),
				output_path: std::path::PathBuf::from(#output_path_string),
				routes_handler: #routes_handler,
			})
		}
		#[cfg(not(debug_assertions))]
		{
			sunfish::Sunfish::Release(sunfish::ReleaseSunfish {
				embedded_dir: #embedded_directory,
				routes: #routes,
				routes_handler: #routes_handler,
			})
		}
	}};
	Ok(code)
}

fn embedded_directory(output_path: &Path) -> proc_macro2::TokenStream {
	let absolute_paths: Vec<PathBuf> = WalkDir::new(&output_path)
		.into_iter()
		.filter_map(|entry| {
			let entry = entry.unwrap();
			let path = entry.path().to_owned();
			let metadata = std::fs::metadata(&path).unwrap();
			if metadata.is_file() {
				Some(path)
			} else {
				None
			}
		})
		.collect();
	let relative_paths: Vec<PathBuf> = absolute_paths
		.iter()
		.map(|absolute_path| absolute_path.strip_prefix(&output_path).unwrap().to_owned())
		.collect();
	let absolute_paths: Vec<String> = absolute_paths
		.into_iter()
		.map(|path| path.to_str().unwrap().to_owned())
		.collect();
	let relative_paths: Vec<String> = relative_paths
		.into_iter()
		.map(|path| path.to_str().unwrap().to_owned())
		.collect();
	quote! {{
		let mut map = std::collections::HashMap::new();
		#({
			let path = std::path::Path::new(#relative_paths);
			let data = include_bytes!(#absolute_paths);
			let hash = sunfish::hash(data);
			let file = sunfish::embed::EmbeddedFile {
				data: data.as_ref(),
				hash,
			};
			map.insert(path, file);
		})*
		sunfish::embed::EmbeddedDirectory::new(map)
	}}
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

fn routes(server_entries: &[ServerEntry]) -> proc_macro2::TokenStream {
	let routes = server_entries
		.iter()
		.map(|server_entry| {
			let package_name = server_entry.package_name.to_owned();
			let package_name_ident = format_ident!("{}", package_name);
			let path_with_placeholders = &server_entry.path_with_placeholders;
			quote! {
				sunfish::Route {
					path_with_placeholders: #path_with_placeholders.to_owned(),
					init: #package_name_ident::init,
				}
			}
		})
		.collect::<Vec<_>>();
	quote! { vec![#(#routes),*] }
}
