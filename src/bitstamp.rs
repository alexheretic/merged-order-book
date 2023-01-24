//! bitstamp exchange.

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use std::{env, time::Duration};
use tokio_tungstenite::tungstenite::Message;

const EXCHANGE_NAME: &str = "bitstamp";

#[derive(Debug)]
pub struct BitstampClient {
    pub tx: tokio::sync::broadcast::Sender<merged_order_book_protos::Summary>,
}

impl BitstampClient {
    /// Connects to bitstamp order book stream.
    ///
    /// Initial future resolves once the first summary has been received.
    pub async fn start(symbol: &str) -> anyhow::Result<Self> {
        let url = env::var("BITSTAMP_URL").unwrap_or_else(|_| "wss://ws.bitstamp.net".into());
        let sub_msg = r#"{"event":"bts:subscribe","data":{"channel":"order_book_"#.to_string()
            + symbol
            + r#""}}"#;

        let (tx, _) = tokio::sync::broadcast::channel(1);
        let tx2 = tx.clone();

        let (connected_tx, connected_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let mut connected = Some(connected_tx);
            loop {
                let (mut ws_write, mut ws_read) = match tokio_tungstenite::connect_async(&url).await
                {
                    Ok((stream, _)) => stream.split(),
                    Err(err) => {
                        eprintln!("{url}: {err}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                if let Err(err) = ws_write.send(Message::Text(sub_msg.clone())).await {
                    eprintln!("bitstamp subscribe {err}");
                    continue;
                }

                while let Some(msg) = ws_read.next().await {
                    let Ok(Message::Text(json)) = msg else { continue };
                    let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) else { continue };
                    if val["event"] != "data" {
                        continue;
                    }
                    let Ok(msg) = serde_json::from_value::<Data>(val) else { continue };
                    match msg.data.try_into() {
                        Ok(summary) => {
                            _ = tx2.send(summary);
                            connected.take().map(|tx| tx.send(()));
                        }
                        Err(_) => {
                            eprintln!("Invalid bitstamp message format `{json}`");
                        }
                    }
                }
            }
        });

        tokio::time::timeout(Duration::from_secs(12), connected_rx)
            .await
            .context("Initial bitstamp connection failed")??;

        eprintln!("Bitstamp connected");

        Ok(Self { tx })
    }
}

/// Data event.
#[derive(Debug, serde::Deserialize)]
struct Data {
    pub data: OrderBook,
}

/// Top 100 bids/asks.
#[derive(Debug, serde::Deserialize)]
struct OrderBook {
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

impl TryFrom<OrderBook> for merged_order_book_protos::Summary {
    type Error = ();

    fn try_from(msg: OrderBook) -> Result<Self, Self::Error> {
        let mut summary = Self {
            bids: msg
                .bids
                .iter()
                .take(10)
                .map(|b| (EXCHANGE_NAME, b).try_into())
                .collect::<Result<_, _>>()?,
            asks: msg
                .asks
                .iter()
                .take(10)
                .map(|b| (EXCHANGE_NAME, b).try_into())
                .collect::<Result<_, _>>()?,
            ..<_>::default()
        };

        if !summary.bids.is_empty() && !summary.asks.is_empty() {
            summary.spread = summary.asks[0].price - summary.bids[0].price;
        }

        Ok(summary)
    }
}
