use crate::util::OrderBook;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Localhost mock bitstamp ws server. Sends messages every ~200ms.
///
/// Subscribe with message `{"event":"bts:subscribe","data":{"channel":"order_book_ethbtc"}}`.
///
/// # Example message
/// ```json
/// {
///     "data": {
///         "timestamp": "1674477880",
///         "microtimestamp": "1674477880557382",
///         "bids": [
///             [
///                 "0.07138988",
///                 "0.60000000"
///             ],...
///         ],
///         "asks": [
///             [
///                 "0.07143677",
///                 "2.56878000"
///             ],...
///         ]
///     },
///     "channel": "order_book_ethbtc",
///     "event": "data"
/// }
/// ```
pub struct MockBitstamp {
    data: Arc<RwLock<OrderBook>>,
    port: u16,
}

impl MockBitstamp {
    pub fn start() -> Self {
        let data = Arc::default();

        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let app = Router::new()
            .route("/", get(ws_handler))
            .with_state(Arc::clone(&data));
        let server = axum::Server::bind(&addr)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>());
        let port = server.local_addr().port();

        tokio::spawn(async move {
            eprintln!("MockBitstamp listening on {}", server.local_addr());
            server.await.unwrap();
        });

        Self { data, port }
    }

    pub fn url(&self) -> String {
        format!("ws://localhost:{}", self.port)
    }

    /// Update order book data. Will be sent on the next websocket update.
    pub fn set_orders(&self, book: OrderBook) {
        *self.data.write().unwrap() = book;
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(data): State<Arc<RwLock<OrderBook>>>,
) -> impl IntoResponse {
    ws.on_upgrade(|ws| connect_ws(ws, data))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn connect_ws(mut ws: WebSocket, data: Arc<RwLock<OrderBook>>) {
    eprintln!("MockBitstamp new connection");

    // await subscribe message
    loop {
        if let Some(msg) = ws.recv().await {
            if let Ok(Message::Text(json)) = msg {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json) {
                    if json["event"] == "bts:subscribe"
                        && json["data"]["channel"] == "order_book_ethbtc"
                    {
                        break; // start publishing
                    }
                }
            }
        } else {
            return; // connection closed
        }
    }

    eprintln!("MockBitstamp publishing on new connection");

    loop {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let msg = {
            let data = data.read().unwrap();
            serde_json::json!({
                "data": {
                    "timestamp": timestamp.as_secs().to_string(),
                    "microtimestamp": timestamp.as_micros().to_string(),
                    "bids": data.bids.iter().map(|o| o.as_array()).collect::<Vec<_>>(),
                    "asks": data.asks.iter().map(|o| o.as_array()).collect::<Vec<_>>(),
                },
                "channel": "order_book_ethbtc",
                "event": "data",
            })
        };

        if ws
            .send(Message::Text(serde_json::to_string(&msg).unwrap()))
            .await
            .is_err()
        {
            return; // connection closed
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
