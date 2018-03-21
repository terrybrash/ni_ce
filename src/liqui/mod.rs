use api::{Header, Headers, HttpClient, HttpRequest, HttpResponse, Method, Payload, Query};
use chrono::Utc;
use failure::{err_msg, Error, ResultExt};
use hex;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal as d128;
use serde::de::{self, Deserialize, DeserializeOwned, Deserializer, Visitor};
use serde_json;
use sha2::Sha512;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use url::Url;

/// Credentials needed for private API requests.
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Credential {
    pub secret: String,
    pub key: String,
    pub nonce: u64,
}

/// `Buy` or `Sell`
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

impl Display for Side {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            Side::Buy => writeln!(f, "buy"),
            Side::Sell => writeln!(f, "sell"),
        }
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Currency(pub String);

impl Display for Currency {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        let &Currency(ref currency) = self;
        f.write_str(currency)
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Serialize)]
pub struct CurrencyPair(pub Currency, pub Currency);

impl Display for CurrencyPair {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        let &CurrencyPair(ref base, ref quote) = self;
        write!(f, "{}_{}", base, quote)
    }
}

impl<'de> Deserialize<'de> for CurrencyPair {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        struct CurrencyPairVisitor;
        impl<'de> Visitor<'de> for CurrencyPairVisitor {
            type Value = CurrencyPair;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("currency pair as a string")
            }

            fn visit_str<E>(self, pair: &str) -> Result<Self::Value, E>
            where E: de::Error {
                let currencies: Vec<&str> = pair.split("_").collect();
                let base = Currency(currencies[0].to_uppercase());
                let quote = Currency(currencies[1].to_uppercase());
                Ok(CurrencyPair(base, quote))
            }
        }
        deserializer.deserialize_str(CurrencyPairVisitor)
    }
}

/// Exchange ticker snapshot.
#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Ticker {
    pub high: d128,
    pub low: d128,
    pub avg: d128,
    pub vol: d128,
    pub vol_cur: d128,
    pub last: d128,
    pub buy: d128,
    pub sell: d128,
    pub updated: u64,
}


/// Market depth.
#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Orderbook {
    pub bids: Vec<(d128, d128)>,
    pub asks: Vec<(d128, d128)>,
}

/// An account's funds, privileges, and number of open orders.
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct AccountInfo {
    /// Your account balance available for trading. Doesn’t include funds on
    /// your open orders.
    pub funds: HashMap<Currency, d128>,

    /// The privileges of the current API key.
    pub rights: Rights,

    /// The number of open orders on this account.
    #[serde(rename = "open_orders")]
    pub num_open_orders: u32,

    /// Server time (UTC).
    pub server_time: i64,
}

/// Account privileges.
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Rights {
    #[serde(rename = "info")]
    pub can_get_info: bool,

    #[serde(rename = "trade")]
    pub can_trade: bool,

    /// Currently unused.
    #[serde(rename = "withdraw")]
    pub can_withdraw: bool,
}

/// The result of a newly placed order.
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct OrderPlacement {
    /// The amount of currency bought/sold.
    pub received: d128,

    /// The remaining amount of currency to be bought/sold (and the initial
    /// order amount).
    pub remains: d128,

    /// Is equal to 0 if the request was fully “matched” by the opposite
    /// orders, otherwise the ID of the executed order will be returned.
    pub order_id: u64,

    /// Balance after the request.
    pub funds: HashMap<Currency, d128>,
}

/// The result of a newly cancelled order.
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct OrderCancellation {
    /// Liqui-issued order id of the cancelled order.
    pub order_id: u64,

    /// Account balance after the order cancellation.
    pub funds: HashMap<Currency, d128>,
}

/// Exchange's time and product info.
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct ExchangeInfo {
    pub server_time: u64,
    #[serde(rename = "pairs")]
    pub products: HashMap<CurrencyPair, ProductInfo>,
}

/// Product min/max prices, trading precision, and fees.
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct ProductInfo {
    /// Maximum number of decimal places allowed for the price(?) and amount(?).
    pub decimal_places: u32,

    /// Minimum price.
    pub min_price: d128,

    /// Maximum price.
    pub max_price: d128,

    /// Minimum buy/sell transaction size.
    pub min_amount: d128,

    /// Whether the pair is hidden. Hidden pairs remain active, but are not displayed on the
    /// exchange's web interface.
    ///
    /// The value is either `0` or `1`. The developers at Liqui don't know booleans exist.
    #[serde(rename = "hidden")]
    pub is_hidden: i32,

    /// Taker fee represented as a fraction of a percent. For example: `taker_fee == 0.25`
    /// represents a 0.25% fee.
    #[serde(rename = "fee")]
    pub taker_fee: d128,
}

/// Status of an order.
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
pub enum OrderStatus {
    Active = 0,
    Executed = 1,
    Cancelled = 2,
    CancelledPartiallyExecuted = 3,
}

/// Limit order (the only type of order Liqui supports).
#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Order {
    pub status: OrderStatus,
    pub pair: CurrencyPair,
    #[serde(rename = "type")]
    pub side: Side,
    pub amount: d128,
    pub rate: d128,
    pub timestamp_created: u64,
}

#[derive(Deserialize, Serialize)]
struct ErrorResponse {
    pub success: i64,
    pub error: String,
}

/// **Public**. Mostly contains product info (min/max price, precision, fees, etc.)
pub fn get_exchange_info<Client>(client: &mut Client, host: &str) -> Result<ExchangeInfo, Error>
where Client: HttpClient {
    let http_request = HttpRequest {
        method: Method::Get,
        host: host,
        path: "/api/3/info",
        body: None,
        query: None,
        headers: None,
    };

    let http_response = client.send(&http_request)?;

    deserialize_public_response(&http_response)
}

/// **Private**. User account information (balances, api priviliges, and more)
pub fn get_account_info<Client>(
    client: &mut Client,
    host: &str,
    credential: &Credential,
) -> Result<AccountInfo, Error>
where
    Client: HttpClient,
{
    let query = {
        let mut query = Query::with_capacity(2);
        query.append_param("method", "getInfo");
        query.append_param("nonce", credential.nonce.to_string());
        query.to_string()
    };
    let headers = private_headers(credential, Some(&query))?;
    let http_request = HttpRequest {
        method: Method::Post,
        host: host,
        path: "/tapi",
        body: Some(query.as_str()),
        headers: Some(headers),
        query: None,
    };
    let http_response = client.send(&http_request)?;
    deserialize_private_response(&http_response)
}

/// **Public**. Market depth.
pub fn get_orderbooks<Client>(
    client: &mut Client,
    host: &str,
    products: &[CurrencyPair],
) -> Result<HashMap<CurrencyPair, Orderbook>, Error>
where
    Client: HttpClient,
{
    let products: Vec<String> = products.iter().map(ToString::to_string).collect();
    let path = ["/api/3/depth/", products.join("-").as_str()].concat();
    let http_request = HttpRequest {
        method: Method::Get,
        host: host,
        path: path.as_str(),
        headers: None,
        body: None,
        query: None,
    };

    let http_response = client.send(&http_request)?;

    deserialize_public_response(&http_response)
}

/// **Public**. Current price/volume ticker.
pub fn get_ticker<Client>(client: &mut Client, host: &str, products: &[CurrencyPair]) -> Result<HashMap<CurrencyPair, Ticker>, Error>
where Client: HttpClient {
    let products: Vec<String> = products.iter().map(ToString::to_string).collect();
    let path = ["/api/3/ticker/", products.join("-").as_str()].concat();
    let http_request = HttpRequest {
        method: Method::Get,
        host: host,
        path: path.as_str(),
        headers: None,
        body: None,
        query: None,
    };

    let http_response = client.send(&http_request)?;

    deserialize_public_response(&http_response)
}

/// **Private**. Place a limit order -- the only order type Liqui supports.
pub fn place_limit_order<Client>(
    client: &mut Client,
    host: &str,
    credential: &Credential,
    product: CurrencyPair,
    price: d128,
    quantity: d128,
    side: Side,
) -> Result<OrderPlacement, Error>
where
    Client: HttpClient,
{
    let body = {
        let mut query = Query::with_capacity(6);
        query.append_param("nonce", credential.nonce.to_string());
        query.append_param("method", "trade");
        query.append_param("pair", product.to_string());
        query.append_param("type", side.to_string());
        query.append_param("rate", price.to_string());
        query.append_param("amount", quantity.to_string());
        query.to_string()
    };
    let headers = private_headers(credential, Some(body.as_str()))?;
    let http_request = HttpRequest {
        method: Method::Post,
        host: host,
        path: "/tapi",
        body: Some(body.as_str()),
        headers: Some(headers),
        query: None,
    };

    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
}

/// **Private**. User's active buy/sell orders for a product.
pub fn get_active_orders<Client>(
    client: &mut Client,
    host: &str,
    credential: &Credential,
    product: CurrencyPair,
) -> Result<HashMap<u64, Order>, Error>
where
    Client: HttpClient,
{
    let body = {
        let mut query = Query::with_capacity(3);
        query.append_param("method", "ActiveOrders");
        query.append_param("nonce", credential.nonce.to_string());
        query.append_param("pair", product.to_string());
        query.to_string()
    };
    let headers = private_headers(credential, Some(body.as_str()))?;
    let http_request = HttpRequest {
        method: Method::Post,
        host: host,
        path: "/tapi",
        body: Some(body.as_str()),
        headers: Some(headers),
        query: None,
    };

    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
}

/// **Private**. Get a specific order by its Liqui-issued order id.
pub fn get_order<Client>(client: &mut Client, host: &str, credential: &Credential, order_id: u64) -> Result<Order, Error>
where Client: HttpClient {
    let body = {
        let mut query = Query::with_capacity(3);
        query.append_param("method", "OrderInfo");
        query.append_param("nonce", credential.nonce.to_string());
        query.append_param("order_id", order_id.to_string());
        query.to_string()
    };
    let headers = private_headers(credential, Some(body.as_str()))?;
    let http_request = HttpRequest {
        method: Method::Post,
        host: host,
        path: "/tapi",
        body: Some(body.as_str()),
        headers: Some(headers),
        query: None,
    };

    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
}

/// **Private**. Cancel an order by its Liqui-issued order id.
pub fn cancel_order<Client>(client: &mut Client, host: &str, credential: &Credential, order_id: u64) -> Result<OrderCancellation, Error>
where Client: HttpClient {
    let body = {
        let mut query = Query::with_capacity(3);
        query.append_param("method", "CancelOrder");
        query.append_param("nonce", credential.nonce.to_string());
        query.append_param("order_id", order_id.to_string());
        query.to_string()
    };
    let headers = private_headers(credential, Some(body.as_str()))?;
    let http_request = HttpRequest {
        method: Method::Post,
        host: host,
        path: "/tapi",
        body: Some(body.as_str()),
        headers: Some(headers),
        query: None,
    };
    
    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
struct PrivateResponse<T> {
    success: i32,
    #[serde(rename = "return")]
    ok: Option<T>,
    error: Option<String>,
    code: Option<u32>,
}

#[derive(Debug, Fail)]
enum PrivateError {
    #[fail(display = "({}) {}", _0, _1)]
    InvalidOrder(u32, String),

    #[fail(display = "({}) {}", _0, _1)]
    InsufficientFunds(u32, String),

    #[fail(display = "({}) {}", _0, _1)]
    OrderNotFound(u32, String),

    #[fail(display = "({:?}) {}", _0, _1)]
    Unregistered(Option<u32>, String),
}

impl<T> PrivateResponse<T> {
    pub fn is_ok(&self) -> bool {
        self.success == 1
    }

    pub fn into_result(self) -> Result<T, PrivateError> {
        if self.is_ok() {
            Ok(self.ok.unwrap())
        } else {
            let error = match self.code {
                Some(code @ 803) | Some(code @ 804) | Some(code @ 805) | Some(code @ 806)
                | Some(code @ 807) => PrivateError::InvalidOrder(code, self.error.unwrap()),

                Some(code @ 831) | Some(code @ 832) => {
                    PrivateError::InsufficientFunds(code, self.error.unwrap())
                }

                Some(code @ 833) => PrivateError::OrderNotFound(code, self.error.unwrap()),

                code => PrivateError::Unregistered(code, self.error.unwrap()),
            };

            Err(error)
        }
    }
}

fn private_headers(credential: &Credential, body: Option<&str>) -> Result<Headers, Error> {
    let mut mac =
        Hmac::<Sha512>::new(credential.secret.as_bytes()).map_err(|e| format_err!("{:?}", e))?;
    if let Some(body) = body {
        mac.input(body.as_bytes());
    }
    let signature = hex::encode(mac.result().code().to_vec());

    let headers = vec![
        Header::new("Key", credential.key.clone()),
        Header::new("Sign", signature),
    ];
    Ok(headers)
}

fn deserialize_private_response<T>(response: &HttpResponse) -> Result<T, Error>
where T: DeserializeOwned {
    let response = match response.body {
        Some(Payload::Text(ref body)) => body,
        Some(Payload::Binary(ref body)) => {
            return Err(format_err!(
                "the response body doesn't contain valid utf8 text: {:?}",
                body
            ))
        }
        None => return Err(err_msg("the body is empty")),
    };

    let response: PrivateResponse<T> = serde_json::from_str(response)
        .with_context(|_| format!("failed to deserialize: \"{}\"", response))?;

    response
        .into_result()
        .map_err(|e| format_err!("the server returned \"{}\"", e))
}

fn deserialize_public_response<T>(response: &HttpResponse) -> Result<T, Error>
where T: DeserializeOwned {
    let response: serde_json::Value = match response.body {
        Some(Payload::Text(ref body)) => serde_json::from_str(body)?,
        Some(Payload::Binary(ref body)) => serde_json::from_slice(body)?,
        None => return Err(err_msg("body is empty")),
    };

    let is_success = response
        .as_object()
        .and_then(|obj| obj.get("success"))
        .and_then(|is_success| is_success.as_u64())
        .map_or(true, |is_success| is_success == 1);

    if is_success {
        let response: T = serde_json::from_value(response)?;
        Ok(response)
    } else {
        let response: ErrorResponse = serde_json::from_value(response)?;
        Err(format_err!("The server returned: {}", response.error))
    }
}
