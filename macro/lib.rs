mod include_dir;
mod init;

#[proc_macro]
pub fn include_dir(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	include_dir::include_dir(input.into())
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

#[proc_macro]
pub fn init(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	init::init(input.into())
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}
