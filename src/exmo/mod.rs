use failure::{Error, ResultExt};
use hex;
use hmac::{Hmac, Mac};
use http;
use rust_decimal::Decimal as d128;
use serde::de::DeserializeOwned;
use serde::de::{Deserialize, Deserializer, Visitor};
use serde;
use serde_json;
use sha2::Sha512;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;
use {HttpClient, Query};

/// Use this as the `host` for REST requests.
pub const API_HOST: &str = "https://api.exmo.com";

/// Credential needed for private API requests.
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Credential {
    pub key: String,
    pub secret: String,
    pub nonce: i64,
}

/// Single currency. `ETH`, `BTC`, `USDT`, etc.
///
/// Use `Currency::from_str` to create a new `Currency`.
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Currency(String);

impl FromStr for Currency {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Currency(s.to_uppercase()))
    }
}

impl Display for Currency {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        let &Currency(ref currency) = self;
        f.write_str(currency)
    }
}

/// Two currencies; `ETH_BTC`, `BTC_USDT`, etc. Usually represents a product.
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Serialize)]
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
        write!(f, "{}_{}", self.base(), self.quote())
    }
}

impl<'de> Deserialize<'de> for CurrencyPair {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        struct CurrencyPairVisitor;
        impl<'de> Visitor<'de> for CurrencyPairVisitor {
            type Value = CurrencyPair;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string containing two currencies separated by an underscore")
            }

            fn visit_str<E>(self, pair: &str) -> Result<Self::Value, E>
            where E: serde::de::Error {
                let currencies: Vec<&str> = pair.split('_').collect();
                if currencies.len() < 2 {
                    return Err(E::invalid_value(serde::de::Unexpected::Str(pair), &self));
                }
                let base = Currency::from_str(currencies[0]).map_err(serde::de::Error::custom)?;
                let quote = Currency::from_str(currencies[1]).map_err(serde::de::Error::custom)?;
                Ok(CurrencyPair(base, quote))
            }
        }
        deserializer.deserialize_str(CurrencyPairVisitor)
    }
}

/// `Buy` or `Sell`
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, PartialEq, Eq, Copy, Hash, PartialOrd, Ord, Clone, Deserialize, Serialize)]
pub enum OrderInstruction {
    LimitBuy,
    LimitSell,
    MarketBuy,
    MarketSell,
    MarketBuyTotal,
    MarketSellTotal,
}

impl Display for OrderInstruction {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            OrderInstruction::LimitBuy => f.write_str("buy"),
            OrderInstruction::LimitSell => f.write_str("sell"),
            OrderInstruction::MarketBuy => f.write_str("market_buy"),
            OrderInstruction::MarketSell => f.write_str("market_sell"),
            OrderInstruction::MarketBuyTotal => f.write_str("market_buy_total"),
            OrderInstruction::MarketSellTotal => f.write_str("market_sell_total"),
        }
    }
}

/// Market depth.
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Orderbook {
    pub ask_quantity: d128,
    pub ask_amount: d128,
    pub ask_top: d128,
    pub bid_quantity: d128,
    pub bid_amount: d128,
    pub bid_top: d128,
    pub ask: Vec<(d128, d128, d128)>,
    pub bid: Vec<(d128, d128, d128)>,
}

/// Private user info (balances, reserved funds, etc.)
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct UserInfo {
    pub uid: i64,
    pub server_date: u64,
    pub balances: HashMap<Currency, d128>,
    pub reserved: HashMap<Currency, d128>,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Order {
    pub order_id: i64,
}

/// **Private**. Get account info (account balances, etc.)
pub fn get_user_info<Client>(
    client: &mut Client,
    host: &str,
    credential: &Credential,
) -> Result<UserInfo, Error>
where
    Client: HttpClient,
{
    let query = {
        let mut query = Query::with_capacity(2);
        query.append_param("nonce", credential.nonce.to_string());
        query.to_string()
    };
    let mut http_request = http::request::Builder::new()
        .method(http::Method::POST)
        .uri(format!("{}/v1/user_info?{}", host, query))
        .body(query)?;
    sign_private_request(&mut http_request, credential)?;

    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
}

/// **Private**. Place a limit order.
pub fn place_limit_order<Client>(
    client: &mut Client,
    host: &str,
    credential: &Credential,
    product: &CurrencyPair,
    price: d128,
    quantity: d128,
    side: Side,
) -> Result<(), Error>
where
    Client: HttpClient,
{
    let query = {
        let mut query = Query::with_capacity(5);
        query.append_param("nonce", credential.nonce.to_string());
        query.append_param("pair", product.to_string());
        query.append_param("quantity", quantity.to_string());
        query.append_param("price", price.to_string());
        match side {
            Side::Buy => query.append_param("type", "buy"),
            Side::Sell => query.append_param("type", "sell"),
        }
        query.to_string()
    };

    let mut http_request = http::request::Builder::new()
        .method(http::Method::POST)
        .uri(format!("{}/v1/order_create?{}", host, query))
        .body(query)?;
    sign_private_request(&mut http_request, credential)?;

    client.send(&http_request)?;

    // Note: Exmo's `Order` doesn't contain anything useful so we don't need
    // to use it.
    Ok(())
}

/// **Public**. Market depth.
pub fn get_orderbooks<Client>(
    client: &mut Client,
    host: &str,
    products: &[&CurrencyPair],
) -> Result<HashMap<CurrencyPair, Orderbook>, Error>
where
    Client: HttpClient,
{
    let products: Vec<String> = products.iter().map(ToString::to_string).collect();
    let query = {
        let mut query = Query::with_capacity(2);
        query.append_param("pair", products.as_slice().join(","));
        query.append_param("limit", "100");
        query.to_string()
    };
    let http_request = http::request::Builder::new()
        .method(http::Method::GET)
        .uri(format!("{}/v1/order_book?{}", host, query))
        .body(String::new())?;

    let http_response = client.send(&http_request)?;

    deserialize_public_response(&http_response)
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
struct ErrorResponse {
    pub result: bool,
    pub error: String,
}

fn sign_private_request(
    request: &mut http::Request<String>,
    credential: &Credential,
) -> Result<(), Error>
{
    let mut mac =
        Hmac::<Sha512>::new(credential.secret.as_bytes()).map_err(|e| format_err!("{:?}", e))?;
    mac.input(request.body().as_bytes());
    let signature = hex::encode(mac.result().code().to_vec());

    let headers = request.headers_mut();
    headers.insert("Key", credential.key.clone().parse().unwrap());
    headers.insert("Sign", signature.parse().unwrap());

    Ok(())
}

/// Deserialize a response returned from a private HTTP request.
fn deserialize_private_response<T>(response: &http::Response<String>) -> Result<T, Error>
where T: DeserializeOwned {
    let body = response.body();
    let response: serde_json::Value = serde_json::from_str(body)?;

    // If the response is an error, it will be a json object containing a
    // `result` equal to `false`.
    let is_error = response
        .as_object()
        .map(|object| {
            match object.get("result") {
                Some(&serde_json::Value::Bool(result)) => !result,
                _ => false,
            }
        })
        .unwrap_or(false);

    if is_error {
        let error: ErrorResponse = serde_json::from_value(response)
            .with_context(|_| format!("failed to deserialize: \"{}\"", body))?;
        Err(format_err!("Server returned: {}", error.error))
    } else {
        let response = serde_json::from_value(response)
            .context(format!("failed to deserialize: \"{}\"", body))?;
        Ok(response)
    }
}

/// Deserialize a response returned from a public HTTP request.
fn deserialize_public_response<T>(response: &http::Response<String>) -> Result<T, Error>
where T: DeserializeOwned {
    let body = response.body();
    Ok(serde_json::from_str(body)?)
}
