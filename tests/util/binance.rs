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
    sync::{
        atomic::{self, AtomicU64},
        Arc, RwLock,
    },
    time::Duration,
};

/// Localhost mock binance ws server. Sends messages every ~100ms.
///
/// # Example `/ws/ethbtc@depth10@100ms` message
/// ```json
/// {
///     "lastUpdateId": 6339921063,
///     "bids": [
///         [
///             "0.07140100",
///             "23.30750000"
///         ],...
///     ],
///     "asks": [
///         [
///             "0.07140200",
///             "8.95600000"
///         ],...
///     ]
///  }
/// ```
pub struct MockBinance {
    data: Arc<RwLock<OrderBook>>,
    port: u16,
}

impl MockBinance {
    pub fn start() -> Self {
        let data = Arc::default();

        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let app = Router::new()
            .route("/ws/ethbtc@depth10@100ms", get(ws_handler))
            .with_state(Arc::clone(&data));
        let server = axum::Server::bind(&addr)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>());
        let port = server.local_addr().port();

        tokio::spawn(async move {
            eprintln!("MockBinance listening on {}", server.local_addr());
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
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    eprintln!("MockBinance publishing on new connection");

    loop {
        let msg = {
            let data = data.read().unwrap();
            serde_json::json!({
                "lastUpdateId": COUNTER.fetch_add(1, atomic::Ordering::Relaxed),
                "bids": data.bids.iter().map(|o| o.as_array()).collect::<Vec<_>>(),
                "asks": data.asks.iter().map(|o| o.as_array()).collect::<Vec<_>>(),
            })
        };

        if ws
            .send(Message::Text(serde_json::to_string(&msg).unwrap()))
            .await
            .is_err()
        {
            return; // connection closed
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
