use quote::quote;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use sha2::Digest;

pub fn hash(bytes: impl AsRef<[u8]>) -> String {
	let mut hash: sha2::Sha256 = Digest::new();
	hash.update(bytes);
	let hash = hash.finalize();
	let hash = hex::encode(hash);
	let hash = &hash[0..16];
	hash.to_owned()
}

pub fn embed(input: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
	let path: syn::LitStr = syn::parse2(input)?;
	let path = Path::new(&path.value()).canonicalize().unwrap();
	let embedded_directory = embedded_directory(&path);
	let code = quote! {{
		#embedded_directory
	}};
	Ok(code)
}

fn embedded_directory(path: &Path) -> proc_macro2::TokenStream {
	let mut absolute_paths: Vec<PathBuf> = WalkDir::new(&path)
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
	absolute_paths.sort();
	let hashes = absolute_paths
		.iter()
		.map(|path| hash(std::fs::read(path).unwrap()));
	let relative_paths = absolute_paths
		.iter()
		.map(|absolute_path| absolute_path.strip_prefix(&path).unwrap().to_owned());
	let absolute_paths = absolute_paths
		.iter()
		.map(|path| path.to_str().unwrap().to_owned());
	let relative_paths = relative_paths.map(|path| path.to_str().unwrap().to_owned());
	quote! {{
		let mut map = std::collections::BTreeMap::new();
		#({
			let path = std::path::Path::new(#relative_paths);
			let data = include_bytes!(#absolute_paths);
			let file = sunfish::embed::EmbeddedFile {
				data: data.as_ref(),
				hash: #hashes,
			};
			map.insert(path, file);
		})*
		sunfish::embed::EmbeddedDirectory(map)
	}}
}
