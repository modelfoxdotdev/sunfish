mod embed;
mod init;

#[proc_macro]
pub fn embed(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	embed::embed(input.into())
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}

#[proc_macro]
pub fn init(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	init::init(input.into())
		.unwrap_or_else(|e| e.to_compile_error())
		.into()
}
