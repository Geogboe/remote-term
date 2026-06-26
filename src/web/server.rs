use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use tower_http::trace::TraceLayer;

use crate::security::token;
use crate::session::{ClientPermit, PtyCommand, SessionState};
use crate::web::{assets, protocol};

#[derive(Clone)]
pub struct AppState {
    pub session: Arc<SessionState>,
}

pub async fn serve(state: AppState, bind_addr: SocketAddr) -> anyhow::Result<()> {
    let listener = bind(bind_addr).await?;
    serve_listener(state, listener).await
}

pub async fn bind(bind_addr: SocketAddr) -> anyhow::Result<tokio::net::TcpListener> {
    Ok(tokio::net::TcpListener::bind(bind_addr).await?)
}

pub async fn serve_listener(
    state: AppState,
    listener: tokio::net::TcpListener,
) -> anyhow::Result<()> {
    axum::serve(listener, router(state)).await?;
    Ok(())
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/t/{token}", get(terminal_page))
        .route("/ws/{token}", get(websocket))
        .route("/assets/main.js", get(main_js))
        .route("/assets/style.css", get(style_css))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn terminal_page(Path(candidate): Path<String>, State(state): State<AppState>) -> Response {
    if !token::is_valid(&candidate, &state.session.token) {
        return StatusCode::NOT_FOUND.into_response();
    }

    Html(assets::INDEX_HTML.replace("__TOKEN__", &candidate)).into_response()
}

async fn main_js() -> Response {
    asset_response("main.js", assets::MAIN_JS)
}

async fn style_css() -> Response {
    asset_response("style.css", assets::STYLE_CSS)
}

fn asset_response(path: &str, body: &'static str) -> Response {
    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(assets::content_type(path)),
    );
    response
}

async fn websocket(
    Path(candidate): Path<String>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    if !token::is_valid(&candidate, &state.session.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let Some(permit) = state.session.try_acquire_client() else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };

    ws.on_upgrade(move |socket| handle_socket(socket, state.session, permit))
}

async fn handle_socket(socket: WebSocket, session: Arc<SessionState>, _permit: ClientPermit) {
    let (mut sender, mut receiver) = socket.split();
    let mut output_rx = session.output_tx.subscribe();

    let frame = match protocol::encode_server_control(&protocol::ServerControl::Status {
        writable: session.web_write,
        word_erase: session.word_erase.clone(),
    }) {
        Ok(frame) => frame,
        Err(_) => return,
    };
    if sender.send(Message::Binary(frame.into())).await.is_err() {
        return;
    }

    let replay = session.scrollback.snapshot();
    if !replay.is_empty()
        && sender
            .send(Message::Binary(protocol::encode_output(&replay).into()))
            .await
            .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            maybe_message = receiver.next() => {
                let Some(Ok(message)) = maybe_message else {
                    break;
                };
                if handle_client_message(message, &session, &mut sender).await.is_err() {
                    break;
                }
            }
            output = output_rx.recv() => {
                match output {
                    Ok(bytes) => {
                        if sender.send(Message::Binary(protocol::encode_output(&bytes).into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

async fn handle_client_message(
    message: Message,
    session: &Arc<SessionState>,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> anyhow::Result<()> {
    match message {
        Message::Binary(bytes) => match protocol::decode_client_frame(&bytes)? {
            protocol::ClientFrame::Input(bytes) => {
                if session.web_write {
                    let _ = session.input_tx.send(PtyCommand::Input(bytes));
                } else {
                    send_error(sender, "browser input is disabled; restart with --write").await?;
                }
            }
            protocol::ClientFrame::Control(protocol::ClientControl::Resize { cols, rows }) => {
                if session.browser_resize {
                    let _ = session.input_tx.send(PtyCommand::Resize { cols, rows });
                }
            }
            protocol::ClientFrame::Control(protocol::ClientControl::Ping) => {
                let frame = protocol::encode_server_control(&protocol::ServerControl::Pong)?;
                sender.send(Message::Binary(frame.into())).await?;
            }
        },
        Message::Close(_) => anyhow::bail!("websocket closed"),
        Message::Ping(bytes) => sender.send(Message::Pong(bytes)).await?,
        Message::Pong(_) => {}
        Message::Text(_) => {
            send_error(sender, "text websocket messages are not supported").await?;
        }
    }

    Ok(())
}

async fn send_error(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &str,
) -> anyhow::Result<()> {
    let frame = protocol::encode_server_control(&protocol::ServerControl::Error {
        message: message.to_string(),
    })?;
    sender.send(Message::Binary(frame.into())).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Method, Request};
    use tower::ServiceExt;

    fn app_state(token: &str) -> AppState {
        let (input_tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let config = crate::session::RunConfig {
            command: vec!["echo".to_string(), "hi".to_string()],
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 7843)),
            lan: false,
            web_write: false,
            max_clients: 1,
            once: false,
            headless: false,
            token: token.to_string(),
            word_erase: vec![0x17],
        };
        AppState {
            session: SessionState::new(&config, input_tx),
        }
    }

    #[tokio::test]
    async fn terminal_page_requires_valid_token() {
        let app = router(app_state("secret"));

        let ok = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/t/secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ok.status(), StatusCode::OK);

        let denied = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/t/wrong")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::NOT_FOUND);
    }
}
