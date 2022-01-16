use futures_batch::ChunksTimeoutStreamExt;
use notify::Watcher;
use std::{convert::Infallible, path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, Notify};
use tokio_stream::StreamExt;
use which::which;

pub struct Config {
	pub host: std::net::IpAddr,
	pub port: u16,
	pub child_host: std::net::IpAddr,
	pub child_port: u16,
	pub watch_paths: Vec<PathBuf>,
	pub ignore_paths: Vec<PathBuf>,
	pub command: String,
}

pub async fn run(config: Config) {
	let Config {
		host,
		port,
		child_host,
		child_port,
		watch_paths,
		ignore_paths,
		command,
	} = config;
	let addr = std::net::SocketAddr::new(host, port);
	let child_addr = std::net::SocketAddr::new(child_host, child_port);
	let cwd = std::env::current_dir().unwrap();
	let watch_paths: Vec<PathBuf> = watch_paths.into_iter().map(|path| cwd.join(path)).collect();
	let ignore_paths: Vec<PathBuf> = ignore_paths
		.into_iter()
		.map(|path| cwd.join(path))
		.collect();

	enum State {
		Ground,
		Building {
			notify: Arc<Notify>,
			child: Option<std::process::Child>,
		},
		Running {
			child: Option<std::process::Child>,
		},
	}
	let state: Arc<Mutex<State>> = Arc::new(Mutex::new(State::Ground));
	let (watch_events_tx, watch_events_rx) = tokio::sync::mpsc::unbounded_channel();
	watch_events_tx.send(()).unwrap();

	// Run the file watcher.
	let mut watcher = notify::recommended_watcher(move |_: notify::Result<notify::Event>| {
		watch_events_tx.send(()).unwrap();
	})
	.unwrap();
	let mut walk_builder = ignore::WalkBuilder::new(watch_paths.first().unwrap());
	for watch_path in watch_paths.iter().skip(1) {
		walk_builder.add(watch_path);
	}
	walk_builder.filter_entry(move |entry| {
		let path = entry.path();
		let ignored = ignore_paths
			.iter()
			.any(|ignore_path| path.starts_with(ignore_path));
		!ignored
	});
	let walk = walk_builder.build();
	for entry in walk {
		let entry = entry.unwrap();
		let path = entry.path();
		watcher
			.watch(path, notify::RecursiveMode::NonRecursive)
			.unwrap();
	}

	tokio::spawn({
		let state = state.clone();
		async move {
			let mut watch_events =
				tokio_stream::wrappers::UnboundedReceiverStream::new(watch_events_rx)
					.chunks_timeout(1_000_000, std::time::Duration::from_millis(10));
			while watch_events.next().await.is_some() {
				// Kill the previous child process if any.
				if let State::Running { child } = &mut *state.lock().await {
					let mut child = child.take().unwrap();
					child.kill().ok();
					child.wait().unwrap();
				}
				// Start the new process.
				let notify = Arc::new(Notify::new());
				let sh = which("sh").unwrap();
				let child = std::process::Command::new(sh)
					.args(vec!["-c", &command])
					.env("HOST", &child_host.to_string())
					.env("PORT", &child_port.to_string())
					.spawn()
					.unwrap();
				*state.lock().await = State::Building {
					notify: notify.clone(),
					child: Some(child),
				};
				loop {
					tokio::time::sleep(std::time::Duration::from_millis(100)).await;
					if let State::Building { child, .. } = &mut *state.lock().await {
						if let Ok(Some(_)) | Err(_) = child.as_mut().unwrap().try_wait() {
							break;
						}
					}
					if tokio::net::TcpStream::connect(&child_addr).await.is_ok() {
						break;
					}
				}
				let child = if let State::Building { child, .. } = &mut *state.lock().await {
					child.take().unwrap()
				} else {
					panic!()
				};
				*state.lock().await = State::Running { child: Some(child) };
				notify.notify_waiters();
			}
		}
	});

	// Handle requests by waiting for a build to finish if one is in progress, then proxying the request to the child process.
	let handler = move |state: Arc<Mutex<State>>, mut request: http::Request<hyper::Body>| async move {
		let notify = if let State::Building { notify, .. } = &mut *state.lock().await {
			Some(notify.clone())
		} else {
			None
		};
		if let Some(notify) = notify {
			notify.notified().await;
		}
		let child_authority = format!("{}:{}", child_host, child_port);
		let child_authority = http::uri::Authority::from_maybe_shared(child_authority).unwrap();
		*request.uri_mut() = http::Uri::builder()
			.scheme("http")
			.authority(child_authority)
			.path_and_query(request.uri().path_and_query().unwrap().clone())
			.build()
			.unwrap();
		hyper::Client::new()
			.request(request)
			.await
			.unwrap_or_else(|_| {
				http::Response::builder()
					.status(http::StatusCode::SERVICE_UNAVAILABLE)
					.body(hyper::Body::from("service unavailable"))
					.unwrap()
			})
	};

	// Start the server.
	let service = hyper::service::make_service_fn(|_| {
		let state = state.clone();
		async move {
			Ok::<_, Infallible>(hyper::service::service_fn(
				move |request: http::Request<hyper::Body>| {
					let state = state.clone();
					async move { Ok::<_, Infallible>(handler(state, request).await) }
				},
			))
		}
	});
	hyper::Server::bind(&addr).serve(service).await.unwrap();
}
