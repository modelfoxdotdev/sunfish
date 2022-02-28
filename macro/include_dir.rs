use quote::quote;
use std::path::Path;

pub fn include_dir(input: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
	let path: syn::LitStr = syn::parse2(input)?;
	let path = Path::new(&path.value()).canonicalize().unwrap();
	let path_string = path.display().to_string();
	let fs_directory = quote! {{
	  let path = std::path::PathBuf::from(#path_string);
		let fs_directory = sunfish::embed::FsDirectory(path);
		sunfish::embed::FsOrEmbeddedDirectory::Fs(fs_directory)
	}};
	let embedded_directory = quote! {{
		let embedded_directory = sunfish::embed!(#path_string);
		sunfish::embed::FsOrEmbeddedDirectory::Embedded(embedded_directory)
	}};
	let include_dir = quote! {{
	  #[cfg(debug_assertions)]
	  let output = #fs_directory;
	  #[cfg(not(debug_assertions))]
	  let output = #embedded_directory;
	  sunfish::include_dir::IncludeDir(output)
	}};
	let code = quote! {{
	  #include_dir
	}};
	Ok(code)
}
