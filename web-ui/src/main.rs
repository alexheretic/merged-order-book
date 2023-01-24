use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use merged_order_book_protos::{
    orderbook_aggregator_client::OrderbookAggregatorClient, Empty, Level,
};
use std::net::SocketAddr;
use tonic::transport::Channel;

#[tokio::main]
async fn main() {
    eprintln!("Connecting to grpc server on 7016");
    let grpc = OrderbookAggregatorClient::connect("http://localhost:7016")
        .await
        .expect("failed to connect to grpc server");

    let addr = SocketAddr::from(([127, 0, 0, 1], 7000));
    let app = Router::new()
        .route("/", get(index_page))
        .route("/ws", get(ws_handler))
        .with_state(grpc);

    eprintln!("Starting server on http://localhost:7000");

    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

async fn index_page() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(client): State<OrderbookAggregatorClient<Channel>>,
) -> impl IntoResponse {
    ws.on_upgrade(|ws| connect_ws(ws, client))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn connect_ws(mut ws: WebSocket, mut client: OrderbookAggregatorClient<Channel>) {
    let mut stream = match client.book_summary(Empty {}).await {
        Ok(s) => s.into_inner(),
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    while let Ok(Some(msg)) = stream.message().await {
        let jmsg = serde_json::json!({
            "spread": msg.spread,
            "bids": msg.bids.into_iter().map(level_json).collect::<Vec<_>>(),
            "asks": msg.asks.into_iter().map(level_json).collect::<Vec<_>>(),
        });

        if ws
            .send(Message::Text(serde_json::to_string(&jmsg).unwrap()))
            .await
            .is_err()
        {
            return; // connection closed
        }
    }
}

fn level_json(l: Level) -> serde_json::Value {
    serde_json::json!({
        "exchange": l.exchange,
        "price": l.price,
        "amount": l.amount,
    })
}
