use serde_json;
use decimal::d128;
use chrono::DateTime;
use chrono::Utc;
use api;
use std::io::{Read, Cursor};
use base64;
use sha2;
use hmac::{Hmac, Mac};

// #[derive(Debug, Copy, Clone)]
// pub enum Environment {
//     Production,
//     Sandbox,
// }

// impl Environment {
//     fn base_address(&self) -> &'static str {
//         match *self {
//             Environment::Production => "https://api.gdax.com",
//             Environment::Sandbox    => "https://api-public.sandbox.gdax.com",
//         }
//     }
// }

// #[derive(Debug, Deserialize)]
// pub struct Product {
//     pub id: String,
//     pub base_currency: String,
//     pub quote_currency: String,
//     pub base_min_size: String,
//     pub base_max_size: String,
//     pub quote_increment: String,
//     pub display_name: String,
//     pub status: String,
//     pub margin_enabled: bool,
//     pub status_message: Option<String>,
// }

// #[derive(Debug, Deserialize)]
// pub struct Ticker {
//     pub trade_id: i64,
//     pub price: String,
//     pub size: String,
//     pub bid: String,
//     pub ask: String,
//     pub volume: String,
//     pub time: String,
// }

// #[derive(Debug, Deserialize)]
// pub struct Trade {
//     pub time: String,
//     pub trade_id: i64,
//     pub price: String,
//     pub size: String,
//     pub side: String,
// }

// #[derive(Debug, Deserialize)]
// pub struct BookLevel1 {
//     pub sequence: i64,
//     pub bids: Vec<(String, String, i64)>,
//     pub asks: Vec<(String, String, i64)>,
// }

// #[derive(Debug, Deserialize)]
// pub struct BookLevel2 {
//     pub sequence: i64,
//     pub bids: Vec<(String, String, i64)>,
//     pub asks: Vec<(String, String, i64)>,
// }

// #[derive(Debug, Deserialize)]
// pub struct BookLevel3 {
//     pub sequence: i64,
//     pub bids: Vec<(String, String, String)>,
//     pub asks: Vec<(String, String, String)>,
// }

// #[derive(Debug, Deserialize)]
// pub struct BidAsk {
//     pub price: f64,
//     pub amount: f64,
// }

// #[derive(Debug, Deserialize)]
// pub struct Book {
//     pub bids: Vec<BidAsk>,
//     pub asks: Vec<BidAsk>,
// }

// #[derive(Debug, Deserialize)]
// pub struct Error {
//     pub message: String,
// }

// #[derive(Debug, Deserialize)]
// pub struct Time {
//     pub iso: String,
//     pub epoch: f64,
// }


#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
pub enum CurrencyPair {
    #[serde(rename = "BTC-USD")] BTCUSD,
    #[serde(rename = "BCH-USD")] BCHUSD,
    #[serde(rename = "LTC-USD")] LTCUSD,
    #[serde(rename = "ETH-USD")] ETHUSD,
    #[serde(rename = "BCH-BTC")] BCHBTC,
    #[serde(rename = "LTC-BTC")] LTCBTC,
    #[serde(rename = "ETH-BTC")] ETHBTC,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum Reason {
    Filled,
    Canceled,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum TimeInForce {
    #[serde(rename="GTC")] GoodTillCanceled,
    #[serde(rename="GTT")] GoodTillTime,
    #[serde(rename="IOC")] ImmediateOrCancel,
    #[serde(rename="FOK")] FillOrKill,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum CancelAfter {
    Min,
    Hour,
    Day,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum OrderStatus {
    Done,
    Settled,
    Open,
    Pending,
    Active,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum Liquidity {
    #[serde(rename="M")] Maker,
    #[serde(rename="T")] Taker,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum SelfTrade {
    #[serde(rename="dc")] DecrementAndCancel,
    #[serde(rename="co")] CancelOldest,
    #[serde(rename="cn")] CancelNewest,
    #[serde(rename="cb")] CancelBoth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="lowercase", tag="type")]
pub enum NewOrder {
    Limit(NewLimitOrder),
    Market(NewMarketOrder),
    Stop(NewStopOrder),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLimitOrder {
    /// Order ID selected by you to identify your order
    pub client_oid: Option<String>,
    pub side: Side,
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub stp: Option<SelfTrade>,

    pub price: d128,
    pub size: d128,
    pub time_in_force: Option<TimeInForce>,
    /// Requires `time_in_force` to be `GTT`
    pub cancel_after: Option<CancelAfter>,
}

/// One of `size` or `funds` is required
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMarketOrder {
    /// Order ID selected by you to identify your order
    pub client_oid: Option<String>,
    pub side: Side,
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub stp: Option<SelfTrade>,

    pub size: Option<d128>,
    pub funds: Option<d128>,
}

/// One of `size` or `funds` is required
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewStopOrder {
    /// Order ID selected by you to identify your order
    pub client_oid: Option<String>,
    pub side: Side,
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub stp: Option<SelfTrade>,

    pub price: d128,
    pub size: Option<d128>,
    pub funds: Option<d128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="lowercase", tag="type")]
pub enum Order {
    Limit(LimitOrder),
    Market(MarketOrder),
    Stop(StopOrder),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitOrder {
    pub id: String,
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub status: OrderStatus,
    pub stp: SelfTrade,
    #[serde(rename="settled")]
    pub is_settled: bool,
    pub side: Side,
    pub created_at: DateTime<Utc>,
    pub filled_size: Option<d128>,
    pub fill_fees: Option<d128>,
    pub done_at: DateTime<Utc>,
    pub done_reason: Reason,

    pub price: d128,
    pub size: d128,
    pub time_in_force: TimeInForce,
    pub cancel_after: Option<CancelAfter>,
    pub post_only: bool,
    pub executed_value: d128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOrder {
    pub id: String,
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub status: OrderStatus,
    pub stp: SelfTrade,
    #[serde(rename="settled")]
    pub is_settled: bool,
    pub side: Side,
    pub created_at: DateTime<Utc>,
    pub filled_size: Option<d128>,
    pub fill_fees: Option<d128>,
    pub done_at: DateTime<Utc>,
    pub done_reason: Reason,

    pub size: Option<d128>,
    pub funds: Option<d128>,
    pub specified_funds: Option<d128>,
    pub executed_value: d128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopOrder {
    pub id: String,
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub status: OrderStatus,
    pub stp: SelfTrade,
    #[serde(rename="settled")]
    pub is_settled: bool,
    pub side: Side,
    pub created_at: DateTime<Utc>,
    pub filled_size: Option<d128>,
    pub fill_fees: Option<d128>,
    pub done_at: DateTime<Utc>,
    pub done_reason: Reason,

    pub price: d128,
    pub size: Option<d128>,
    pub funds: Option<d128>,
}

type Credential<'a> = (&'a str, &'a str, &'a str);

trait NeedsAuthentication<C>: Sized {
    fn authenticate(self, credential: C) -> PrivateRequest<Self, C> {
        PrivateRequest {
            credential: credential,
            request: self,
        }
    }
}

pub struct PrivateRequest<R, C> {
    pub request: R,
    pub credential: C,
}

impl<'a> NeedsAuthentication<Credential<'a>> for NewOrder {}
impl<'a> api::Api for PrivateRequest<NewOrder, Credential<'a>> {
    type Reply = Order;
    type Error = serde_json::Error;
    type Body = Cursor<Vec<u8>>;

    fn method(&self) -> api::Method {
        api::Method::Post
    }

    fn path(&self) -> String {
        format!("/orders")
    }

    fn body(&self) -> Self::Body {
        Cursor::new(serde_json::to_vec(&self.request).unwrap())
    }

    fn headers(&self) -> api::Headers {
        private_headers(self, self.credential)
    }

    fn parse<R>(&self, response: &mut R) -> Result<Self::Reply, Self::Error> where R: api::HttpResponse {
        serde_json::from_reader(response.body())
    }
}

// pub fn private_headers(request_path: &str, body: &str, (key, secret, passphrase): Credential) -> api::Headers {
pub fn private_headers<R>(request: &R, (key, secret, password): Credential) -> api::Headers where R: api::Api {
    let mut body = String::new();
    request.body().read_to_string(&mut body).unwrap();
    let timestamp = Utc::now().timestamp().to_string();
    let hmac_key = base64::decode(secret).unwrap();
    let mut signature = Hmac::<sha2::Sha256>::new(&hmac_key).unwrap();
    signature.input(format!("{}{}{}{}", timestamp, request.method(), request.path(), body).as_bytes());
    let signature = base64::encode(&signature.result().code());

    let mut headers = api::Headers::with_capacity(4);
    headers.insert("CB-ACCESS-KEY".to_owned(), vec![key.to_owned()]);
    headers.insert("CB-ACCESS-SIGN".to_owned(), vec![signature]);
    headers.insert("CB-ACCESS-TIMESTAMP".to_owned(), vec![timestamp]);
    headers.insert("CB-ACCESS-PASSPHRASE".to_owned(), vec![password.to_owned()]);
    headers
}