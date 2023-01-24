use merged_order_book_protos::Summary;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Multiple same-currency summary merging broadcaster.
///
/// Merges all summaries into a combined top 10.
#[derive(Debug)]
pub struct Top10SummaryMerger {
    pub tx: broadcast::Sender<Summary>,
}

impl Top10SummaryMerger {
    /// Listen to multiple broadcaster merging and re-broadcasting.
    ///
    /// Note: All summaries must be the same currencies.
    pub fn listen_to(rx: Vec<broadcast::Receiver<Summary>>) -> Self {
        let (tx, _) = broadcast::channel(1);

        let all = Arc::new(Mutex::new(vec![Summary::default(); rx.len()]));

        for (idx, mut rx) in rx.into_iter().enumerate() {
            let all = Arc::clone(&all);
            let tx = tx.clone();
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(summary) => {
                            let mut all = all.lock().unwrap();
                            all[idx] = summary;
                            _ = tx.send(merge_summaries(&all));
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            eprintln!("channel closed");
                            break;
                        }
                    }
                }
            });
        }

        Self { tx }
    }
}

fn merge_summaries(sums: &[Summary]) -> Summary {
    let mut merged = Summary::default();
    for s in sums {
        merged.bids.extend(s.bids.clone());
        merged.asks.extend(s.asks.clone());
    }

    // sort bids so highest price with highest amount is top
    merged.bids.sort_by(|a, b| {
        a.price
            .total_cmp(&b.price)
            .reverse()
            .then(a.amount.total_cmp(&b.amount).reverse())
    });
    // sort asks so lowest price with highest amount is top
    merged.asks.sort_by(|a, b| {
        a.price
            .total_cmp(&b.price)
            .then(a.amount.total_cmp(&b.amount).reverse())
    });

    if !merged.bids.is_empty() && !merged.asks.is_empty() {
        merged.spread = merged.asks[0].price - merged.bids[0].price;
    }

    merged.bids.truncate(10);
    merged.asks.truncate(10);

    merged
}
