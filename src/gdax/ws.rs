use decimal::d128;
use url::Url;
use chrono::{DateTime, Utc};
use api;
use gdax::{CurrencyPair, Currency, Side, Credential};
use serde_json;
use base64;
use hmac::{Hmac, Mac};
use sha2;

pub fn production() -> Url {
    Url::parse("wss://ws-feed.gdax.com").unwrap()
}

pub fn sandbox() -> Url {
    Url::parse("wss://ws-feed-public.sandbox.gdax.com").unwrap()
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ChannelName {
    Level2,
    Heartbeat,
    Ticker,
    Full,
    User,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Channel {
    pub name: ChannelName,
    #[serde(rename = "product_ids")] 
    pub products: Vec<CurrencyPair>,
}

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum Message {
    Error(Error),
    Subscribe(Subscribe),
    Subscriptions(Subscriptions),
    Unsubscribe(Unsubscribe),
    Heartbeat(Heartbeat),
    Ticker(Ticker),
    Snapshot(Snapshot),
    L2Update(L2Update),

    /// A valid order has been received and is now active. This message is
    /// emitted for every single valid order as soon as the matching engine
    /// receives it whether is fills immediately or not.
    Received(Order),
    
    /// The order is now open on the order book. This message will only be
    /// sent for orders which are not fully filled immediately.
    /// `remaining_size` will indicate how much of the order is unfilled and
    /// going on the book.
    Open(Order),

    /// The order is no longer on the order book. Sent for all orders for
    /// which there was a received message. This message can result from an
    /// order being cancelled or filled. There will be no more messages for
    /// this `order_id` after a done message. `remaining_size` indicates how
    /// much of the order went unfilled; this will be 0 for **filled** orders.
    ///
    /// "Market" orders will not have a`remaining_size` or a `price` field as
    /// "they are never on the open order book at a given price.
    Done(Order),

    /// A trade occurred between two orders. The aggressor or **taker** order
    /// is the one executing immediately after being received and the
    /// **maker** order is a resting order on the book. The `side` field
    /// inidcates the maker order side. If the side is `sell` this indicates
    /// the maker was a sell order and the match is considered an ip-tick. A
    /// `buy` side match is a down-tick.
    Match(Order),

    /// An order has changed. This is the result of self-trade prevention
    /// adjusting the order size or available funds. Orders can only decrease
    /// in size or funds. **change** messages are sent anytime an order
    /// changes in size; this includes resting orders (open) as well as
    /// received but not yet open. **change** messages are also sent when a
    /// new market order goes through self trade prevention and the **funds**
    /// for the market order have changed.
    Change(Order),

    /// An activate message is sent when a stop order is placed. When the stop
    /// is triggered, the order will be placed and o through the "order
    /// lifecycle".
    Activate(Order),
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Order {
    // Received and Open fields
    order_id: Option<String>,
    time: DateTime<Utc>,
    product_id: CurrencyPair,
    sequence: Option<i64>,
    size: Option<d128>,
    price: Option<d128>,
    side: Side,
    order_type: Option<OrderType>,
    
    // Done fields
    remaining_size: Option<d128>,
    reason: Option<OrderReason>,
    
    // Change fields
    new_size: Option<d128>,
    old_size: Option<d128>,

    // Match fields
    taker_order_id: Option<String>,
    maker_order_id: Option<String>,
    trade_id: Option<i64>,
    taker_user_id: Option<String>,
    user_id: Option<String>,
    taker_profile_id: Option<String>,
    profile_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
#[serde(rename_all="lowercase")]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
#[serde(rename_all="lowercase")]
pub enum OrderReason {
    Filled,
    Canceled,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Error {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Subscribe {
    #[serde(rename = "product_ids")] 
    pub products: Vec<CurrencyPair>,
    pub channels: Vec<Channel>,
    pub key: String,
    pub timestamp: String,
    pub signature: String,
    pub passphrase: String,
}

impl Subscribe {
    pub fn new(products: &[CurrencyPair], channels: &[Channel], credential: &Credential) -> Self {
        let timestamp = Utc::now().timestamp().to_string();
        let hmac_key = base64::decode(&credential.secret).unwrap();
        let mut signature = Hmac::<sha2::Sha256>::new(&hmac_key).unwrap();
        signature.input(format!("{}{}{}", timestamp, "GET", "/users/self/verify").as_bytes());
        let signature = base64::encode(&signature.result().code());

        Subscribe {
            products: products.to_vec(),
            channels: channels.to_vec(),
            key: credential.key.clone(),
            timestamp: timestamp,
            signature: signature,
            passphrase: credential.password.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Subscriptions {
    pub channels: Vec<Channel>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Unsubscribe {
    #[serde(rename = "product_ids")] 
    pub products: Option<Vec<CurrencyPair>>,
    pub channels: Vec<ChannelName>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Heartbeat {
    pub sequence: i64,
    pub last_trade_id: i64,
    #[serde(rename = "product_id")] 
    pub product: CurrencyPair,
    pub time: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
pub struct Ticker {
    pub trade_id: i64,
    pub sequence: i64,
    pub time: DateTime<Utc>,
    #[serde(rename = "product_id")] 
    pub product: CurrencyPair,
    pub price: d128,
    #[serde(rename = "side")] 
    pub taker_side: Side,
    pub last_size: d128,
    pub best_bid: d128,
    pub best_ask: d128,
}

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
pub struct Snapshot {
    #[serde(rename = "product_id")] 
    pub product: CurrencyPair,
    pub bids: Vec<(d128, d128)>,
    pub asks: Vec<(d128, d128)>,
}

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
pub struct L2Update {
    #[serde(rename = "product_id")] 
    pub product: CurrencyPair,
    pub changes: Vec<(Side, d128, d128)>,
    pub time: DateTime<Utc>,
}

impl api::WebsocketResource for Subscribe {
    type Message = Message;
    type Error = serde_json::Error;

    fn method(&self) -> api::Method {
        api::Method::Get
    }

    fn path(&self) -> String {
        // This makes no sense...but it's what's required for authenticating on websocket connections.
        // This isn't even fucking documented, I had to find this on an unofficial GitHub project
        // stupid stupid stupid stupid stupid
        "/users/self/verify".to_owned()
    }

    fn serialize(message: Self::Message) -> Result<api::WebsocketMessage, Self::Error> {
        Ok(api::WebsocketMessage::Text(serde_json::to_string(&message)?))
    }

    fn deserialize(message: api::WebsocketMessage) -> Result<Self::Message, Self::Error> {
        match message {
            api::WebsocketMessage::Text(message) => {
                serde_json::from_str(&message)
            }
            _ => unimplemented!(),
        }
    }
}
