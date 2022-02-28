pub use self::{
	builder::{build, BuildOptions},
	hash::hash,
};
use anyhow::Result;
use embed::IncludeDir;
use futures::FutureExt;
use std::{future::Future, path::Path, pin::Pin};
pub use sunfish_macro::{embed, include_dir, init};

mod builder;
pub mod embed;
mod hash;
pub mod watchserve;

pub enum Route {
	Static {
		paths: Option<Box<dyn 'static + Send + Sync + Fn() -> Vec<String>>>,
		handler: Box<dyn 'static + Send + Sync + Fn(String) -> String>,
	},
	Dynamic {
		handler: DynamicHandler,
	},
}

pub type DynamicHandler = Box<
	dyn Send + Sync + for<'a> Fn(&'a mut http::Request<hyper::Body>) -> DynamicHandlerOutput<'a>,
>;

pub type DynamicHandlerOutput<'a> =
	Pin<Box<dyn 'a + Send + Future<Output = Result<http::Response<hyper::Body>>>>>;

impl Route {
	pub fn new_static<H>(handler: H) -> Route
	where
		H: 'static + Send + Sync + Fn(String) -> String,
	{
		Route::Static {
			paths: None,
			handler: Box::new(handler),
		}
	}

	pub fn new_static_with_paths<P, H>(paths: P, handler: H) -> Route
	where
		P: 'static + Send + Sync + Fn() -> Vec<String>,
		H: 'static + Send + Sync + Fn(String) -> String,
	{
		Route::Static {
			paths: Some(Box::new(paths)),
			handler: Box::new(handler),
		}
	}

	pub fn new_dynamic<H>(handler: H) -> Route
	where
		H: 'static
			+ Send
			+ Sync
			+ for<'a> Fn(&'a mut http::Request<hyper::Body>) -> DynamicHandlerOutput<'a>,
	{
		Route::Dynamic {
			handler: Box::new(handler),
		}
	}

	pub fn handle<'a>(
		&self,
		request: &'a mut http::Request<hyper::Body>,
	) -> DynamicHandlerOutput<'a> {
		match self {
			Route::Static { handler, .. } => {
				let html = handler(request.uri().path().to_owned());
				async {
					let response = http::Response::builder()
						.status(http::StatusCode::OK)
						.body(hyper::Body::from(html))
						.unwrap();
					Ok(response)
				}
				.boxed()
			}
			Route::Dynamic { handler } => handler(request),
		}
	}
}

pub fn path_components(path: &str) -> Vec<&str> {
	path.split('/').skip(1).collect::<Vec<_>>()
}

pub fn asset_path(path: &Path) -> String {
	let extension = path.extension().map(|e| e.to_str().unwrap()).unwrap();
	let hash = hash(&path.to_str().unwrap().as_bytes());
	format!("/assets/{}.{}", hash, extension)
}

pub struct ClientPaths {
	pub path_js: String,
	pub path_wasm: String,
}

pub fn client_paths(crate_name: &'static str) -> ClientPaths {
	let hash = hash(crate_name.as_bytes());
	ClientPaths {
		path_js: format!("/js/{}.js", hash),
		path_wasm: format!("/js/{}_bg.wasm", hash),
	}
}

type RoutesHandler = Box<
	dyn Send + Sync + for<'a> Fn(&'a mut http::Request<hyper::Body>) -> RoutesHandlerOutput<'a>,
>;

type RoutesHandlerOutput<'a> =
	Pin<Box<dyn 'a + Send + Future<Output = Result<Option<http::Response<hyper::Body>>>>>>;

pub struct Sunfish {
	pub output: IncludeDir,
	pub routes_handler: RoutesHandler,
}

pub struct RouteInitializer {
	pub path_with_placeholders: String,
	pub init: fn() -> Route,
}

impl Sunfish {
	pub async fn handle(
		&self,
		request: &mut http::Request<hyper::Body>,
	) -> Result<Option<http::Response<hyper::Body>>> {
		let response = self.serve_page(request).await?;
		let response = match response {
			Some(response) => Some(response),
			None => self.serve_asset(request).await?,
		};
		Ok(response)
	}

	async fn serve_page(
		&self,
		request: &mut http::Request<hyper::Body>,
	) -> Result<Option<http::Response<hyper::Body>>> {
		self.routes_handler.as_ref()(request).await
	}

	async fn serve_asset(
		&self,
		request: &http::Request<hyper::Body>,
	) -> Result<Option<http::Response<hyper::Body>>> {
		let method = request.method().clone();
		let uri = request.uri().clone();
		let path_and_query = uri.path_and_query().unwrap();
		let path = path_and_query.path();
		if method != ::http::Method::GET {
			return Ok(None);
		}
		let path = Path::new(path.strip_prefix('/').unwrap());
		let file = if let Some(file) = self.output.read(path) {
			file
		} else {
			return Ok(None);
		};
		let mut response = http::Response::builder();
		if let Some(content_type) = content_type(path) {
			response = response.header(http::header::CONTENT_TYPE, content_type);
		}
		if let Some(hash) = file.hash() {
			response = response.header(http::header::ETAG, hash);
			if let Some(etag) = request.headers().get(http::header::IF_NONE_MATCH) {
				if etag.as_bytes() == hash.as_bytes() {
					response = response.status(http::StatusCode::NOT_MODIFIED);
					let response = response.body(hyper::Body::empty()).unwrap();
					return Ok(Some(response));
				}
			}
		}
		response = response.status(http::StatusCode::OK);
		let response = response.body(hyper::Body::from(file.data())).unwrap();
		Ok(Some(response))
	}
}

fn content_type(path: &std::path::Path) -> Option<&'static str> {
	let path = path.to_str().unwrap();
	if path.ends_with(".css") {
		Some("text/css")
	} else if path.ends_with(".js") {
		Some("text/javascript")
	} else if path.ends_with(".svg") {
		Some("image/svg+xml")
	} else if path.ends_with(".wasm") {
		Some("application/wasm")
	} else {
		None
	}
}
