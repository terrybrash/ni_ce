use chrono;
use decimal::d128;
use url::Url;
use {ConnectionInfo, Header};

pub fn production() -> Url {
    Url::parse("wss://ws-feed.gdax.com").unwrap()
}

pub fn sandbox() -> Url {
    Url::parse("wss://ws-feed-public.sandbox.gdax.com").unwrap()
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
pub enum CurrencyPair {
    #[serde(rename="BTC-USD")] BTCUSD,
    #[serde(rename="BCH-USD")] BCHUSD,
    #[serde(rename="LTC-USD")] LTCUSD,
    #[serde(rename="ETH-USD")] ETHUSD,
    #[serde(rename="BCH-BTC")] BCHBTC,
    #[serde(rename="LTC-BTC")] LTCBTC,
    #[serde(rename="ETH-BTC")] ETHBTC,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ChannelName {
    Level2,
    Heartbeat,
    Ticker,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Channel {
    pub name: ChannelName,
    #[serde(rename="product_ids")]
    pub products: Vec<CurrencyPair>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase", tag="type")]
pub enum Message {
    Error(Error),
    Subscribe(Subscribe),
    Subscriptions(Subscriptions),
    Unsubscribe(Unsubscribe),
    Heartbeat(Heartbeat),
    Ticker(Ticker),
    Snapshot(Snapshot),
    L2Update(L2Update),
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Error {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Subscribe {
    #[serde(rename="product_ids")]
    pub products: Vec<CurrencyPair>,
    pub channels: Vec<Channel>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Subscriptions {
    pub channels: Vec<Channel>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Unsubscribe {
    #[serde(rename="product_ids")]
    pub products: Option<Vec<CurrencyPair>>,
    pub channels: Vec<ChannelName>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Heartbeat {
    pub sequence: i64,
    pub last_trade_id: i64,
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub time: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
pub struct Ticker {
    pub trade_id: i64,
    pub sequence: i64,
    pub time: chrono::DateTime<chrono::Utc>,
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub price: d128,
    #[serde(rename="side")]
    pub taker_side: Side,
    pub last_size: d128,
    pub best_bid: d128,
    pub best_ask: d128,
}

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
pub struct Snapshot {
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub bids: Vec<(d128, d128)>,
    pub asks: Vec<(d128, d128)>,
}

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
pub struct L2Update {
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub changes: Vec<(Side, d128, d128)>,
    pub time: chrono::DateTime<chrono::Utc>,
}


pub fn connect(base_address: Url) -> ConnectionInfo {
    ConnectionInfo {
        address: base_address,
        headers: None,
    }
}

// pub fn subscribe(channels: Vec<mpsc::Sender<model::ExchangeUpdate>>, product: model::CurrencyPair) {
//     let (mut socket, _) = tungstenite::connect(url::Url::parse(WebSocketBaseAddress::PRODUCTION.unwrap()).unwrap()).unwrap();

//     let subscribe_message = Message::Subscribe(Subscribe {
//         product_ids: Vec::new(),
//         channels: vec![
//             Channel {
//                 name: ChannelName::Level2,
//                 product_ids: vec![str_from_product(product).to_owned()],
//             }
//         ],
//     });

//     socket.write_message(tungstenite::Message::text(serde_json::to_string(&subscribe_message).unwrap())).unwrap();

//     while let Ok(message) = socket.read_message() {
//         match message {
//             tungstenite::Message::Text(text) => {
//                 let message: Message = serde_json::from_str(&text).unwrap();
//                 for update in updates_from_message(message) {
//                     for channel in &channels {
//                         channel.send(update.clone()).unwrap();
//                     }
//                 }
//             },
//             _ => println!("unhandled"),
//         }
//     }
// }
