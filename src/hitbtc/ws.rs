use serde::ser::Serialize;
use serde::de::{DeserializeOwned, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Request<T> {
    /// An identifier established by the Client that **MUST** contain a `String`, `Number`, or `NULL` value if included.
    /// If it is not included it is assumed to be a notification. The value **SHOULD** normally not be `NULL`.
    ///
    /// The Server **MUST** reply with the same value in the `Response` object if included. this 
    /// member is used to correlate the context between the two objects.
    pub id: Option<i64>,
    /// **MUST** be exactly "2.0"
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<T>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Response<T, E> {
    /// This member is **REQUIRED**.
    /// It **MUST** be the same as the value of the id member in the `Request` object.
    /// If there was an error in detecting the id in the `Request` object (e.g. parse error/invalid 
    /// request), it **MUST** be `NULL`.
    pub id: Option<i64>,
    /// **MUST** be exactly "2.0"
    pub jsonrpc: String,
    /// This member is **REQUIRED** on success.
    /// This member **MUST NOT** exist if there was an error invoking the method.
    pub result: Option<T>,
    /// This member is **REQUIRED** on error.
    /// This member **MUST NOT** exist if there was no error triggered during invocation.
    pub error: Option<Error<E>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Error<T> {
    pub code: i64,
    pub message: String,
    pub data: Option<T>,
}

trait RequestParams {}
trait ResponseResult {}
trait NotificationParams {}

impl RequestParams for GetCurrencyParams {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GetCurrencyParams {
    pub currency: String,
}

impl RequestParams for GetSymbolParams {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GetSymbolParams {
    pub symbol: String,
}

impl RequestParams for SubscribeTickerParams {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SubscribeTickerParams {
    pub symbol: String,
}

impl RequestParams for SubscribeOrderbookParams {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SubscribeOrderbookParams {
    pub symbol: String,
}

impl RequestParams for SubscribeTradesParams {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SubscribeTradesParams {
    pub symbol: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum SortOrder {
    #[serde(rename = "DESC")]
    Descending,
    #[serde(rename = "ASC")]
    Ascending,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum SortBy {
    #[serde(rename = "timestamp")]
    Timestamp,
    #[serde(rename = "id")]
    Id,
}

impl RequestParams for GetTradesParams {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GetTradesParams {
    pub symbol: Option<String>,
    pub limit: Option<i64>,
    pub sort: Option<SortOrder>,
    pub by: Option<SortBy>,
    pub from: Option<String>,
    pub till: Option<String>,
    pub offset: Option<i64>,
}

impl RequestParams for SubscribeCandlesParams {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SubscribeCandlesParams {
    pub symbol: String,
    pub period: String,
}

impl ResponseResult for Currency {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Currency {
    pub id: String,
    pub full_name: String,
    pub crypto: bool,
    pub payin_enabled: bool,
    pub payin_payment_id: bool,
    pub payin_confirmations: i64,
    pub payout_enabled: bool,
    pub payout_is_payment_id: bool,
    pub transfer_enabled: bool,
}

impl ResponseResult for Symbol {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub quantity_increment: String,
    pub tick_size: String,
    pub take_liquidity_rate: String,
    pub provide_liquidity_rate: String,
    pub fee_currency: String,
}

impl NotificationParams for Ticker {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    pub ask: String,
    pub bid: String,
    pub last: String,
    pub open: String,
    pub low: String,
    pub high: String,
    pub volume: String,
    pub volume_quote: String,
    pub timestamp: String,
    pub symbol: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BidAsk {
    pub price: String,
    pub size: String,
}

impl NotificationParams for Orderbook {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Orderbook {
    pub ask: Vec<BidAsk>,
    pub bid: Vec<BidAsk>,
    pub symbol: String,
    pub sequence: i64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub id: i64,
    pub price: String,
    pub quantity: String,
    pub side: String,
    pub timestamp: String,
}

impl NotificationParams for Trades {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Trades {
    pub data: Vec<Trade>,
    pub symbol: String,
}

impl NotificationParams for Candles {}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Candles {
    pub data: Vec<Candle>,
    pub symbol: String,
    pub period: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Candle {
    pub timestamp: String,
    pub open: String,
    pub close: String,
    pub min: String,
    pub max: String,
    pub volume: String,
    pub volume_quote: String,
}

