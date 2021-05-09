use market_finance::Bar as MarketBar;
use market_finance::Timestamped;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug)]
pub struct Bar {
    /// A single piece of OHLC information
    pub bar: MarketBar,

    /// The price at the end of the unit of time, adjusted to include dividends
    pub adjusted_close: f64,
}

impl Eq for Bar {}
impl Ord for Bar {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bar.timestamp.cmp(&other.bar.timestamp)
    }
}
impl PartialOrd for Bar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Bar {
    fn eq(&self, other: &Self) -> bool {
        self.bar.timestamp == other.bar.timestamp
    }
}
impl Timestamped for Bar {
    /// Gets the timestamp in millisecond accuracy
    fn timestamp_millis(&self) -> i64 {
        self.bar.timestamp
    }
}
