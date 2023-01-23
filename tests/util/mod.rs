use std::time::Duration;

pub mod binance;
pub mod bitstamp;

/// A generally "long enough" time to wait for an async thing to have happened.
pub const TEST_WAIT: Duration = Duration::from_secs(4);

/// Returns a random currently available port.
pub async fn random_open_port() -> u16 {
    let addr = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    addr.local_addr().unwrap().port()
}

#[derive(Debug, Default)]
pub struct OrderBook {
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,
}

#[derive(Debug)]
pub struct Order {
    pub price: String,
    pub amount: String,
}

impl Order {
    pub fn as_array(&self) -> [&str; 2] {
        [&self.price, &self.amount]
    }
}

impl From<[&str; 2]> for Order {
    fn from([price, amount]: [&str; 2]) -> Self {
        Self {
            price: price.into(),
            amount: amount.into(),
        }
    }
}
