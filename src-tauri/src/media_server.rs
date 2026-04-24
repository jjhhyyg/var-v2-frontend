use crate::*;

pub(crate) fn start_media_server(
    tokens: Arc<RwLock<HashMap<String, MediaToken>>>,
) -> anyhow::Result<u16> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = std_listener.local_addr()?.port();
    std_listener.set_nonblocking(true)?;

    let app = Router::new()
        .route("/media/{token}", any(media_handler))
        .with_state(MediaServerState { tokens });

    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                backend_log_error(format!("failed to create media runtime: {error}"));
                return;
            }
        };

        runtime.block_on(async move {
            let listener = match tokio::net::TcpListener::from_std(std_listener) {
                Ok(listener) => listener,
                Err(error) => {
                    backend_log_error(format!("failed to create media listener: {error}"));
                    return;
                }
            };

            if let Err(error) = axum::serve(listener, app).await {
                backend_log_error(format!("media server stopped: {error}"));
            }
        });
    });

    Ok(port)
}

#[derive(Clone)]
pub(crate) struct MediaServerState {
    pub(crate) tokens: Arc<RwLock<HashMap<String, MediaToken>>>,
}

async fn media_handler(
    AxumState(state): AxumState<MediaServerState>,
    AxumPath(token): AxumPath<String>,
    request: Request<Body>,
) -> Response {
    let now = Instant::now();
    let path = {
        let tokens = state.tokens.read();
        tokens
            .get(&token)
            .filter(|media| media.expires_at > now)
            .map(|media| media.path.clone())
    };

    match path {
        Some(path) => match ServeFile::new(path).oneshot(request).await {
            Ok(response) => response.into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
