use base64::decode;
use futures::{future, SinkExt, Stream, StreamExt};
use serde::Serialize;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use crate::error::InnerError;

use crate::yahoo::{PricingData, PricingData_MarketHoursType};
use crate::TradingSession;

use super::Quote;

#[derive(Debug, Clone, Serialize)]
struct Subs {
    subscribe: Vec<String>,
}

fn convert_session(value: PricingData_MarketHoursType) -> TradingSession {
    match value {
        PricingData_MarketHoursType::PRE_MARKET => TradingSession::PreMarket,
        PricingData_MarketHoursType::REGULAR_MARKET => TradingSession::Regular,
        PricingData_MarketHoursType::POST_MARKET => TradingSession::AfterHours,
        _ => TradingSession::Other,
    }
}

/// Realtime price quote streamer
///
/// To use it:
/// 1. Create a new streamer with `Streamer::new().await;`
/// 1. Subscribe to some symbols with `streamer.subscribe(vec!["AAPL"], |quote| /* do something */).await;`
/// 1. Let the streamer run `streamer.run().await;`
pub struct Streamer {
    subs: Vec<String>,
    shutdown: Arc<Mutex<bool>>,
}
impl Streamer {
    pub fn new(symbols: Vec<&str>) -> Streamer {
        let mut subs = Vec::new();
        for symbol in &symbols {
            subs.push(symbol.to_string());
        }

        Streamer {
            subs,
            shutdown: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn stream(&self) -> impl Stream<Item = Result<Quote, InnerError>> {
        let (tx, rx) = mpsc::channel();

        let (stream, _) = connect_async("wss://streamer.finance.yahoo.com")
            .await.unwrap();
        let (mut sink, source) = stream.split();

        // send the symbols we are interested in streaming
        let message = serde_json::to_string(&Subs {
            subscribe: self.subs.clone(),
        })
        .unwrap();
        tx.send(Message::Text(message)).unwrap();

        // spawn a separate thread for sending out messages
        let shutdown = self.shutdown.clone();
        tokio::spawn(async move {
            loop {
                // stop on shutdown notification
                if *(shutdown.lock().unwrap()) {
                    break;
                }

                // we're still running - so get a message and send it out.
                let res_msg = rx.try_recv();
                if let Ok(msg) = res_msg {
                    sink.send(msg).await.unwrap();
                } else {
                    tokio::time::sleep(Duration::from_millis(333)).await;
                }
            }
        });

        let pong_tx = tx.clone();
        let shutdown = self.shutdown.clone();
        source
            .filter_map(move |msg| {
                match msg {
                    Ok(ok_msg) => {
                        match ok_msg {
                            Message::Ping(_) => match pong_tx.send(Message::Pong("pong".as_bytes().to_vec())) {
                                Ok(_) => {}
                                Err(e) => return future::ready(Some(Err(InnerError::SendError{ source: e } )))
                            }
                            Message::Close(_) => {
                                *(shutdown.lock().unwrap()) = true;
                            }
                            Message::Text(value) => {
                                return future::ready(Some(Ok(value)));
                            }
                            Message::Binary(value) => {
                                return future::ready(Some(Ok(String::from_utf8(value).unwrap())));
                            }
                            _ => {}
                        }
                    }
                    Err(e) => return future::ready(Some(Err(InnerError::SocketError{ source: e })))
                };
                return future::ready(None);
            })
            .map(move |msg| {
                let data: PricingData = protobuf::Message::parse_from_bytes(&decode(&msg?).unwrap().as_bytes()).unwrap();
                Ok(
                    Quote {
                        symbol: data.id.to_string(),
                        timestamp: data.time as i64,
                        session: convert_session(data.marketHours),
                        price: data.price as f64,
                        volume: data.dayVolume as u64,
                    }
                )
            })
    }

    pub fn stop(&self) {
        let mut shutdown = self.shutdown.lock().unwrap();
        *shutdown = true;
    }
}
