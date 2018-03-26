//! [Binance.com](https://binance.com) API.
use {HttpClient, Query};
use chrono::Utc;
use failure::Error;
use hex;
use serde_json;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal as d128;
use serde::de::DeserializeOwned;
use sha2::Sha256;
use std::fmt::{self, Display, Formatter};
use http;

/// Use this as the `host` for REST requests.
pub const API_HOST: &str = "https://api.binance.com";

/// API key and secret. Required for private API calls.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Credential {
    pub secret: String,
    pub key: String,
}

/// General exchange info; rate limits, products, filters, etc.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeInfo {
    pub timezone: String,
    pub server_time: u64,
    pub rate_limits: Vec<RateLimit>,
    pub exchange_filters: Vec<Filter>,
    #[serde(rename = "symbols")]
    pub products: Vec<ProductInfo>,
}

/// Symbol info; base, quote, precision, status, etc.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProductInfo {
    /// This is `base` and `quote` concatenated. This can't be processed into an actual
    /// `CurrencyPair` since there's no seperator.
    pub symbol: String,
    pub status: SymbolStatus,
    #[serde(rename = "baseAsset")]
    pub base: Currency,
    #[serde(rename = "baseAssetPrecision")]
    pub base_precision: u32,
    #[serde(rename = "quoteAsset")]
    pub quote: Currency,
    pub quote_precision: u32,
    pub order_types: Vec<OrderInstruction>,
    pub iceberg_allowed: bool,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[serde(rename_all = "UPPERCASE")]
pub enum SymbolStatus {
    Trading,
}

/// Order type. `Limit`, `Market`, etc.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderInstruction {
    Limit,
    LimitMaker,
    Market,
    StopLossLimit,
    TakeProfitLimit,
}

impl Display for OrderInstruction {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        use self::OrderInstruction::*;
        match *self {
            Limit => f.write_str("LIMIT"),
            LimitMaker => f.write_str("LIMIT_MAKER"),
            Market => f.write_str("MARKET"),
            StopLossLimit => f.write_str("STOP_LOSS_LIMIT"),
            TakeProfitLimit => f.write_str("TAKE_PROFIT_LIMIT"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "filterType")]
pub enum Filter {
    #[serde(rename_all = "camelCase")]
    PriceFilter {
        min_price: d128,
        max_price: d128,
        tick_size: d128,
    },

    #[serde(rename_all = "camelCase")]
    LotSize {
        #[serde(rename = "minQty")]
        min_quantity: d128,
        #[serde(rename = "maxQty")]
        max_quantity: d128,
        step_size: d128,
    },

    #[serde(rename_all = "camelCase")]
    MinNotional { min_notional: d128 },
}

/// Interval of time. Mostly used in [`RateLimit`].
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum Interval {
    Second,
    Minute,
    Day,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "rateLimitType")]
pub enum RateLimit {
    #[serde(rename_all = "camelCase")]
    Requests { interval: Interval, limit: u32 },

    #[serde(rename_all = "camelCase")]
    Orders { interval: Interval, limit: u32 },
}

/// Account balances, priviliges, fee rates, etc.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Maker fee in percentage of 1%, represented as `0..100`.
    #[serde(rename = "makerCommission")]
    pub maker_fee: i32,

    /// Taker fee in percentage of 1%, represented as `0..100`.
    #[serde(rename = "takerCommission")]
    pub taker_fee: i32,

    /// Buyer fee in percentage of 1%, represented as `0..100`.
    #[serde(rename = "buyerCommission")]
    pub buyer_fee: i32,

    /// Seller fee in percentage of 1%, represented as `0..100`.
    #[serde(rename = "sellerCommission")]
    pub seller_fee: i32,
    pub can_trade: bool,
    pub can_withdraw: bool,
    pub can_deposit: bool,
    pub update_time: i64,
    pub balances: Vec<Balance>,
}

/// Account balance.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    #[serde(rename = "asset")]
    pub currency: Currency,

    /// Available for trading.
    pub free: d128,

    /// Locked (not sure when this would happen)
    pub locked: d128,
}

/// Market depth.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Orderbook {
    pub last_update_id: u64,
    /// Vector of `(price, quantity, /*ignore this*/)`
    pub asks: Vec<(d128, d128, [(); 0])>,

    /// Vector of `(price, quantity, /*ignore this*/)`
    pub bids: Vec<(d128, d128, [(); 0])>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Order {}

/// Result of a `cancel_order` request.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrderCancellation {
    pub symbol: String,
    pub orig_client_order_id: String,
    pub order_id: u64,
    pub client_order_id: String,
}

/// `Buy` or `Sell`
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Buy,
    Sell,
}

impl Display for Side {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            Side::Buy => f.write_str("BUY"),
            Side::Sell => f.write_str("SELL"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum TimeInForce {
    #[serde(rename = "IOC")]
    ImmediateOrCancel,
    #[serde(rename = "GTC")]
    GoodTillCancelled,
    #[serde(rename = "FOK")]
    FillOrKill,
}

impl Display for TimeInForce {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        use self::TimeInForce::*;
        match *self {
            ImmediateOrCancel => f.write_str("IOC"),
            GoodTillCancelled => f.write_str("GTC"),
            FillOrKill => f.write_str("FOK"),
        }
    }
}

/// A single currency. `ETH`, `BTC`, `USDT`, etc.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Currency(String);

impl Currency {
    pub fn from_str(string: &str) -> Self {
        Currency(string.to_uppercase())
    }
}

impl Display for Currency {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        let &Currency(ref currency) = self;
        f.write_str(currency.as_str())
    }
}

/// Usually represents a product. `ETH_BTC`, `BTC_USDT`, etc.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct CurrencyPair(pub Currency, pub Currency);

impl CurrencyPair {
    /// Convenience method for accessing the base currency when `CurrencyPair` represents a
    /// product.
    pub fn base(&self) -> &Currency {
        let &CurrencyPair(ref base, _) = self;
        base
    }

    /// Convenience method for accessing the quote currency when `CurrencyPair` represents a
    /// product.
    pub fn quote(&self) -> &Currency {
        let &CurrencyPair(_, ref quote) = self;
        quote
    }
}

impl Display for CurrencyPair {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}{}", self.base(), self.quote())
    }
}

/// **Private**. Get priviliges, commission rates, and balances for an account.
pub fn get_account_info<Client>(
    client: &mut Client,
    host: &str,
    credential: &Credential,
) -> Result<Account, Error>
where
    Client: HttpClient,
{
    let query = {
        let mut query = Query::with_capacity(2);
        query.append_param("timestamp", timestamp_now().to_string());
        let signature = private_signature(credential, query.to_string().as_str())?;
        query.append_param("signature", signature);
        query.to_string()
    };
    let http_request = http::request::Builder::new()
        .method(http::Method::GET)
        .uri(format!("{}/api/v3/account?{}", host, query))
        .header(X_MBX_APIKEY, credential.key.as_str())
        .body(String::new())?;

    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
}

/// **Public**.
pub fn get_exchange_info<Client>(client: &mut Client, host: &str) -> Result<ExchangeInfo, Error>
where Client: HttpClient {
    let http_request = http::request::Builder::new()
        .method(http::Method::GET)
        .uri(format!("{}/api/v1/exchangeInfo", host))
        .body(String::new())?;

    let http_response = client.send(&http_request)?;

    deserialize_public_response(&http_response)
}

/// **Public**. Get the orderbook for a single product.
pub fn get_orderbook<Client>(
    client: &mut Client,
    host: &str,
    product: &CurrencyPair,
) -> Result<Orderbook, Error>
where
    Client: HttpClient,
{
    let query = {
        let mut query = Query::with_capacity(2);
        query.append_param("symbol", product.to_string());
        query.append_param("limit", "100");
        query.to_string()
    };
    let http_request = http::request::Builder::new()
        .method(http::Method::GET)
        .uri(format!("{}/api/v1/depth?{}", host, query))
        .body(String::new())?;

    let http_response = client.send(&http_request)?;

    deserialize_public_response(&http_response)
}

/// **Private**. Place a limit order.
pub fn place_limit_order<Client>(
    client: &mut Client,
    host: &str,
    credential: &Credential,
    product: &CurrencyPair,
    price: d128,
    quantity: d128,
    time_in_force: TimeInForce,
    side: Side,
) -> Result<Order, Error>
where
    Client: HttpClient,
{
    let query = {
        let mut query = Query::with_capacity(7);
        query.append_param("timestamp", timestamp_now().to_string());
        query.append_param("symbol", product.to_string());
        query.append_param("side", side.to_string());
        query.append_param("type", OrderInstruction::Limit.to_string());
        query.append_param("quantity", quantity.to_string());
        query.append_param("price", price.to_string());
        query.append_param("timeInForce", time_in_force.to_string());
        let signature = private_signature(credential, query.to_string().as_str())?;
        query.append_param("signature", signature);
        query.to_string()
    };
    let http_request = http::request::Builder::new()
        .method(http::Method::POST)
        .uri(format!("{}/api/v3/order?{}", host, query))
        .header(X_MBX_APIKEY, credential.key.as_str())
        .body(String::new())?;

    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
}

/// **Private**. Cancel an active order by Binance-issued order id.
pub fn cancel_order<Client>(
    client: &mut Client,
    host: &str,
    credential: &Credential,
    order_id: u64,
    product: &CurrencyPair,
) -> Result<OrderCancellation, Error>
where
    Client: HttpClient,
{
    let query = {
        let mut query = Query::with_capacity(5);
        query.append_param("timestamp", timestamp_now().to_string());
        query.append_param("symbol", product.to_string());
        query.append_param("orderId", order_id.to_string());
        let signature = private_signature(credential, query.to_string().as_str())?;
        query.append_param("signature", signature);
        query.to_string()
    };
    let http_request = http::request::Builder::new()
        .method(http::Method::DELETE)
        .uri(format!("{}/api/v3/order?{}", host, query))
        .header(X_MBX_APIKEY, credential.key.as_str())
        .body(String::new())?;

    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
}

/// **Private**. Get all open orders for every product or all open orders for one product.
pub fn get_open_orders<Client>(
    client: &mut Client,
    host: &str,
    credential: &Credential,
    product: Option<CurrencyPair>,
) -> Result<Vec<Order>, Error>
where
    Client: HttpClient,
{
    let query = {
        let mut query = Query::with_capacity(5);
        query.append_param("timestamp", timestamp_now().to_string());
        if let Some(product) = product {
            query.append_param("symbol", product.to_string());
        }
        let signature = private_signature(credential, query.to_string().as_str())?;
        query.append_param("signature", signature);
        query.to_string()
    };
    let http_request = http::request::Builder::new()
        .method(http::Method::GET)
        .uri(format!("{}/api/v3/openOrders?{}", host, query))
        .header(X_MBX_APIKEY, credential.key.as_str())
        .body(String::new())?;

    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
}

fn timestamp_now() -> u64 {
    let now = Utc::now();
    // now.timestamp() as u64 * 1000 + now.timestamp_subsec_millis() as u64
    now.timestamp() as u64 * 1000
}

fn private_signature(credential: &Credential, query: &str) -> Result<String, Error> {
    let mut mac =
        Hmac::<Sha256>::new(credential.secret.as_bytes()).map_err(|e| format_err!("{:?}", e))?;
    mac.input(query.as_bytes());
    Ok(hex::encode(mac.result().code().to_vec()))
}

const X_MBX_APIKEY: &str = "X-MBX-APIKEY";

fn deserialize_private_response<T>(response: &http::Response<String>) -> Result<T, Error>
where T: DeserializeOwned {
    deserialize_public_response(response)
}

fn deserialize_public_response<T>(response: &http::Response<String>) -> Result<T, Error>
where T: DeserializeOwned {
    let result = serde_json::from_str(response.body().as_str())?;
    Ok(result)
}
