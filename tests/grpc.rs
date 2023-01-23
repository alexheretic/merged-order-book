use crate::util::{binance::MockBinance, bitstamp::MockBitstamp, OrderBook, TEST_WAIT};
use approx::assert_relative_eq;
use merged_order_book_protos::orderbook_aggregator_client::OrderbookAggregatorClient;
use std::{
    env,
    time::{Duration, Instant},
};

mod util;

macro_rules! assert_level_eq {
    ($lvl:expr, $name:expr, $price:expr, $amount:expr) => {{
        let lvl = &$lvl;
        assert_eq!(lvl.exchange, $name);
        assert_relative_eq!(lvl.price, $price);
        assert_relative_eq!(lvl.amount, $amount);
    }};
}

/// Scenario test for the grpc service.
///
/// Asserts that binance & bitstamp order book streams are listened
/// to & merged properly into the grpc interface.
#[tokio::test]
async fn grpc() {
    // start and setup binance & bitstamp ws
    const HIGH_BID: &str = "0.07140100";
    const HIGH_BID_F: f64 = 0.07140100;
    const MID_BID: &str = "0.07138988";
    const MID_BID_F: f64 = 0.07138988;
    const LOW_BID: &str = "0.07138900";
    const LOW_BID_F: f64 = 0.07138900;

    const LOW_ASK: &str = "0.07143677";
    const LOW_ASK_F: f64 = 0.07143677;
    const MID_ASK: &str = "0.07143800";
    const MID_ASK_F: f64 = 0.07143800;
    const HIGH_ASK: &str = "0.07150000";
    const HIGH_ASK_F: f64 = 0.07150000;

    let binance = MockBinance::start();
    binance.set_orders(OrderBook {
        bids: vec![
            [HIGH_BID, "23.30750000"].into(),
            [LOW_BID, "10.50000000"].into(),
        ],
        asks: vec![
            [MID_ASK, "14.56878000"].into(),
            [HIGH_ASK, "2.50000000"].into(),
        ],
    });

    let bitstamp = MockBitstamp::start();
    bitstamp.set_orders(OrderBook {
        bids: vec![
            [MID_BID, "0.60000000"].into(),
            [LOW_BID, "1.20000000"].into(),
        ],
        asks: vec![
            [LOW_ASK, "2.56878000"].into(),
            [HIGH_ASK, "10.10000000"].into(),
        ],
    });

    // configure & start grpc server
    env::set_var("BINANCE_URL", binance.url());
    env::set_var("BITSTAMP_URL", bitstamp.url());
    let grpc_port = util::random_open_port().await;
    env::set_var("GRPC_PORT", grpc_port.to_string());
    tokio::spawn(async {
        if let Err(err) = merged_order_book_grpc::start().await {
            eprintln!("{err}");
        }
    });

    // await a grpc connection
    let a = Instant::now();
    let mut client = loop {
        let c = OrderbookAggregatorClient::connect(format!("http://localhost:{grpc_port}")).await;
        if let Ok(client) = c {
            break client;
        }
        assert!(a.elapsed() < TEST_WAIT);
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    let mut stream = client
        .book_summary(merged_order_book_protos::Empty {})
        .await
        .expect("book_summary")
        .into_inner();

    // await a message with both exchanges inside
    let a = Instant::now();
    let msg = loop {
        let next = stream.message().await.unwrap();
        let next = next.expect("stream closed");

        if next.bids.iter().any(|b| b.exchange == "binance")
            && next.bids.iter().any(|b| b.exchange == "bitstamp")
        {
            break next;
        }
        assert!(a.elapsed() < TEST_WAIT);
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    eprintln!("{msg:#?}");

    let bids = &msg.bids;
    assert_level_eq!(bids[0], "binance", HIGH_BID_F, 23.3075);
    assert_level_eq!(bids[1], "bitstamp", MID_BID_F, 0.6);
    assert_level_eq!(bids[2], "binance", LOW_BID_F, 10.5);
    assert_level_eq!(bids[3], "bitstamp", LOW_BID_F, 1.2);

    let asks = &msg.asks;
    assert_level_eq!(asks[0], "bitstamp", LOW_ASK_F, 2.56878);
    assert_level_eq!(asks[1], "binance", MID_ASK_F, 14.56878);
    assert_level_eq!(asks[2], "bitstamp", HIGH_ASK_F, 10.1);
    assert_level_eq!(asks[3], "binance", HIGH_ASK_F, 2.5);

    assert_relative_eq!(msg.spread, LOW_ASK_F - HIGH_BID_F);

    // bitstamp updates with a new lower ask
    const LOWER_ASK: &str = "0.07143300";
    const LOWER_ASK_F: f64 = 0.071433;

    bitstamp.set_orders(OrderBook {
        bids: vec![
            [MID_BID, "0.60000000"].into(),
            [LOW_BID, "1.20000000"].into(),
        ],
        asks: vec![
            [LOWER_ASK, "10.10000000"].into(), // changed
            [LOW_ASK, "2.56878000"].into(),
        ],
    });

    // await a new message
    let a = Instant::now();
    let msg = loop {
        let next = stream.message().await.unwrap();
        let next = next.expect("stream closed");

        if next != msg {
            break next;
        }
        assert!(a.elapsed() < TEST_WAIT);
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    eprintln!("{msg:#?}");

    let bids = &msg.bids;
    assert_level_eq!(bids[0], "binance", HIGH_BID_F, 23.3075);
    assert_level_eq!(bids[1], "bitstamp", MID_BID_F, 0.6);
    assert_level_eq!(bids[2], "binance", LOW_BID_F, 10.5);
    assert_level_eq!(bids[3], "bitstamp", LOW_BID_F, 1.2);

    let asks = &msg.asks;
    assert_level_eq!(asks[0], "bitstamp", LOWER_ASK_F, 10.1);
    assert_level_eq!(asks[1], "bitstamp", LOW_ASK_F, 2.56878);
    assert_level_eq!(asks[2], "binance", MID_ASK_F, 14.56878);
    assert_level_eq!(asks[3], "binance", HIGH_ASK_F, 2.5);

    assert_relative_eq!(msg.spread, LOWER_ASK_F - HIGH_BID_F);

    // binance updates with a new higher bid
    const HIGHER_BID: &str = "0.07143000";
    const HIGHER_BID_F: f64 = 0.07143;

    binance.set_orders(OrderBook {
        bids: vec![
            [HIGHER_BID, "10.50000000"].into(), // changed
            [HIGH_BID, "23.30750000"].into(),
        ],
        asks: vec![
            [MID_ASK, "14.56878000"].into(),
            [HIGH_ASK, "2.50000000"].into(),
        ],
    });

    // await a new message
    let a = Instant::now();
    let msg = loop {
        let next = stream.message().await.unwrap();
        let next = next.expect("stream closed");

        if next != msg {
            break next;
        }
        assert!(a.elapsed() < TEST_WAIT);
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    eprintln!("{msg:#?}");

    let bids = &msg.bids;
    assert_level_eq!(bids[0], "binance", HIGHER_BID_F, 10.5);
    assert_level_eq!(bids[1], "binance", HIGH_BID_F, 23.3075);
    assert_level_eq!(bids[2], "bitstamp", MID_BID_F, 0.6);
    assert_level_eq!(bids[3], "bitstamp", LOW_BID_F, 1.2);

    let asks = &msg.asks;
    assert_level_eq!(asks[0], "bitstamp", LOWER_ASK_F, 10.1);
    assert_level_eq!(asks[1], "bitstamp", LOW_ASK_F, 2.56878);
    assert_level_eq!(asks[2], "binance", MID_ASK_F, 14.56878);
    assert_level_eq!(asks[3], "binance", HIGH_ASK_F, 2.5);

    assert_relative_eq!(msg.spread, LOWER_ASK_F - HIGHER_BID_F);
}
