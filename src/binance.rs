//! binance exchange.

use anyhow::Context;
use futures_util::StreamExt;
use std::{env, time::Duration};
use tokio_tungstenite::tungstenite::Message;

const EXCHANGE_NAME: &str = "binance";

#[derive(Debug)]
pub struct BinanceClient {
    pub tx: tokio::sync::broadcast::Sender<merged_order_book_protos::Summary>,
}

impl BinanceClient {
    /// Connects to bitstamp order book stream.
    ///
    /// Initial future resolves once the first summary has been received.
    pub async fn start(symbol: &str) -> anyhow::Result<Self> {
        let url = env::var("BINANCE_URL")
            .unwrap_or_else(|_| "wss://stream.binance.com:9443".into())
            + "/ws/"
            + symbol
            + "@depth10@100ms";

        let (tx, _) = tokio::sync::broadcast::channel(1);
        let tx2 = tx.clone();

        let (connected_tx, connected_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let mut connected = Some(connected_tx);
            loop {
                let (_, mut ws_read) = match tokio_tungstenite::connect_async(&url).await {
                    Ok((stream, _)) => stream.split(),
                    Err(err) => {
                        eprintln!("{url}: {err}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                while let Some(msg) = ws_read.next().await {
                    let Ok(Message::Text(json)) = msg else { continue };
                    let Ok(msg) = serde_json::from_str::<DepthMessage>(&json) else { continue };
                    match msg.try_into() {
                        Ok(summary) => {
                            _ = tx2.send(summary);
                            connected.take().map(|tx| tx.send(()));
                        }
                        Err(_) => {
                            eprintln!("Invalid binance message format `{json}`");
                        }
                    }
                }
            }
        });

        tokio::time::timeout(Duration::from_secs(12), connected_rx)
            .await
            .context("Initial binance connection failed")??;

        eprintln!("Binance connected");

        Ok(Self { tx })
    }
}

/// Top 10 bids/asks.
#[derive(Debug, serde::Deserialize)]
struct DepthMessage {
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

impl TryFrom<DepthMessage> for merged_order_book_protos::Summary {
    type Error = ();

    fn try_from(msg: DepthMessage) -> Result<Self, Self::Error> {
        let mut summary = Self {
            bids: msg
                .bids
                .iter()
                .map(|b| (EXCHANGE_NAME, b).try_into())
                .collect::<Result<_, _>>()?,
            asks: msg
                .asks
                .iter()
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
