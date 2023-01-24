mod binance;
mod bitstamp;
mod merger;

use crate::{binance::BinanceClient, bitstamp::BitstampClient, merger::SummaryMerger};
use futures_util::{Stream, StreamExt};
use merged_order_book_protos::orderbook_aggregator_server::OrderbookAggregatorServer;
use std::{env, pin::Pin};
use tokio_stream::wrappers::BroadcastStream;
use tonic::Status;

/// Starts the grpc server & connects to binance & bitstamp ethbtc exchanges.
pub async fn start() -> anyhow::Result<()> {
    // connect to exchanges & await first message concurrently
    let (binance, bitstamp) = futures_util::try_join!(
        BinanceClient::start("ethbtc"),
        BitstampClient::start("ethbtc"),
    )?;
    let merger = SummaryMerger::listen_to(vec![binance.tx.subscribe(), bitstamp.tx.subscribe()]);

    let service = OrderbookAggregatorServer::new(GrcServer { events: merger });

    let port: u16 = env::var("GRPC_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7016);

    eprintln!("Starting grpc server on {port}");

    tonic::transport::Server::builder()
        .add_service(service)
        .serve(std::net::SocketAddr::from(([127, 0, 0, 1], port)))
        .await?;

    Ok(())
}

#[derive(Debug)]
pub struct GrcServer {
    events: SummaryMerger,
}

#[tonic::async_trait]
impl merged_order_book_protos::orderbook_aggregator_server::OrderbookAggregator for GrcServer {
    type BookSummaryStream =
        Pin<Box<dyn Stream<Item = Result<merged_order_book_protos::Summary, Status>> + Send>>;

    async fn book_summary(
        &self,
        _: tonic::Request<merged_order_book_protos::Empty>,
    ) -> Result<tonic::Response<Self::BookSummaryStream>, tonic::Status> {
        let rx = self.events.tx.subscribe();

        let out = BroadcastStream::new(rx).filter_map(|r| {
            std::future::ready(match r {
                Ok(r) => Some(Ok::<_, _>(r)),
                _ => None, // ignore lagged messages
            })
        });

        Ok(tonic::Response::new(
            Box::pin(out) as Self::BookSummaryStream
        ))
    }
}
