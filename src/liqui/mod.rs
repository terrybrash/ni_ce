use api::{Header, Headers, HttpClient, HttpRequest, HttpResponse, Method, Payload, Query};
use chrono::Utc;
use failure::{err_msg, Error, ResultExt};
use hex;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal as d128;
use serde::de::{self, Deserializer, DeserializeOwned, Deserialize, Visitor};
use serde_json;
use sha2::Sha512;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use url::Url;

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Credential {
    pub secret: String,
    pub key: String,
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
pub enum PrivateError {
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

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Orderbook {
    pub bids: Vec<(d128, d128)>,
    pub asks: Vec<(d128, d128)>,
}

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

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct OrderPlacement {
    /// The amount of currency bought/sold.
    pub received: d128,

    /// The remaining amount of currency to be bought/sold (and the initial
    /// order amount).
    pub remains: d128,

    /// Is equal to 0 if the request was fully “matched” by the opposite
    /// orders, otherwise the ID of the executed order will be returned.
    pub order_id: i64,

    /// Balance after the request.
    pub funds: HashMap<Currency, d128>,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct ExchangeInfo {
    pub server_time: u64,
    #[serde(rename = "pairs")]
    pub products: HashMap<CurrencyPair, ProductInfo>,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct ProductInfo {
    pub decimal_places: u32,
    pub min_price: d128,
    pub max_price: d128,
    pub min_amount: d128,
    pub hidden: i32,
    pub fee: d128,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Order {
    pub status: i32,
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
        query.append_param("nonce", nonce().to_string());
        query.to_string()
    };
    let headers = private_headers(credential, Some(&query))?;
    let http_request = HttpRequest {
        method: Method::Post,
        host: host,
        path: "/tapi",
        body: Some(Payload::Text(query)),
        headers: Some(headers),
        query: None,
    };
    let http_response = client.send(&http_request)?;
    deserialize_private_response(&http_response)
}

// pub fn get_balances<Client>(
//     client: &mut Client,
//     host: &str,
//     credential: &ccex::Credential,
// ) -> Result<HashMap<ccex::Currency, d128>, Error>
// where
//     Client: HttpClient,
// {
//     get_info(client, host, credential)?.funds.into_iter()
//         // If a currency can't be converted, it means it's been newly
//         // added to Liqui and hasn't been added to the `Currency` enum. In
//         // that case, ignoring it is fine.
//         .filter_map(|(currency, balance)| {
//             match ccex::Currency::try_from(currency) {
//                 Ok(currency) => Some((currency, balance)),
//                 Err(_) => None,
//             }
//         })
//         .map(|(currency, balance)| {
//             let balance = d128::from_f64(balance)
//                 .ok_or_else(|| format_err!("Couldn't convert {} into a decimal", balance))?;
//             Ok((currency, balance))
//         })
//         .collect()
// }

/// **Public**. Market depth.
pub fn get_orderbooks<Client>(
    client: &mut Client,
    host: &str,
    products: &[CurrencyPair],
) -> Result<HashMap<CurrencyPair, Orderbook>, Error>
where
    Client: HttpClient,
{
    // let products: Vec<String> = products
    //     .iter()
    //     .map(|product| {
    //         let product = CurrencyPair::try_from(*product)?;
    //         Ok(product.to_string())
    //     })
    //     .collect::<Result<Vec<String>, Error>>()?;

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
    // deserialize_public_response::<HashMap<CurrencyPair, Orderbook>>(&http_response)?
    //     .into_iter()
    //     .filter_map(|(product, orderbook)| {
    //         let product: Result<ccex::CurrencyPair, Error> = product.try_into();
    //         match product {
    //             Ok(product) => Some((product, orderbook)),
    //             Err(_) => {
    //                 // If we get a product back that we don't support, just silently filter it.
    //                 None
    //             }
    //         }
    //     })
    //     .map(|(product, orderbook)| {
    //         let asks: Result<ccex::Asks, Error> = orderbook
    //             .asks
    //             .iter()
    //             .map(|&(price, amount)| {
    //                 let price = d128::from_f64(price)
    //                     .ok_or_else(|| format_err!("Couldn't convert {} into a decimal", price))?;
    //                 let amount = d128::from_f64(amount)
    //                     .ok_or_else(|| format_err!("Couldn't convert {} into a decimal", amount))?;
    //                 Ok(ccex::Offer::new(price, amount))
    //             })
    //             .collect();
    //         let bids: Result<ccex::Bids, Error> = orderbook
    //             .bids
    //             .iter()
    //             .map(|&(price, amount)| {
    //                 let price = d128::from_f64(price)
    //                     .ok_or_else(|| format_err!("Couldn't convert {} into a decimal", price))?;
    //                 let amount = d128::from_f64(amount)
    //                     .ok_or_else(|| format_err!("Couldn't convert {} into a decimal", amount))?;
    //                 Ok(ccex::Offer::new(price, amount))
    //             })
    //             .collect();
    //         Ok((product, ccex::Orderbook::new(asks?, bids?)))
    //     })
    //     .collect()
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
        query.append_param("nonce", nonce().to_string());
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
        body: Some(Payload::Text(body)),
        headers: Some(headers),
        query: None,
    };

    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
    // let placed_order: OrderPlacement = deserialize_private_response(&http_response)?;
    // let placed_order = ccex::Order {
    //     id: None,        //Some(placed_order.order_id),
    //     server_id: None, //Some(placed_order.order_id.to_string()),
    //     side: side,
    //     product: product,
    //     status: ccex::OrderStatus::Open,
    //     instruction: ccex::OrderInstruction::Limit {
    //         price,
    //         original_quantity: d128::from_f64(placed_order.received).unwrap()
    //             + d128::from_f64(placed_order.remains).unwrap(),
    //         remaining_quantity: d128::from_f64(placed_order.remains).unwrap(),
    //         time_in_force: ccex::TimeInForce::GoodTillCancelled,
    //     },
    // };
    // Ok(placed_order)
}

/// **Private**. User's active buy/sell orders for a product.
pub fn get_orders<Client>(client: &mut Client, host: &str, credential: &Credential, product: CurrencyPair)
-> Result<Vec<Order>, Error> 
where Client: HttpClient {
    let query = {
        let mut query = Query::with_capacity(3);
        query.append_param("method", "ActiveOrders");
        query.append_param("nonce", nonce().to_string());
        query.append_param("pair", product.to_string());
        query.to_string()
    };
    let headers = private_headers(credential, Some(query.as_str()))?;
    let http_request = HttpRequest {
        method: Method::Post,
        host: host,
        path: "/tapi",
        body: Some(Payload::Text(query.to_string())),
        headers: Some(headers),
        query: None,
    };

    let http_response = client.send(&http_request)?;

    deserialize_private_response(&http_response)
}

// fn orders(&mut self, product: ccex::CurrencyPair) -> Result<Vec<ccex::Order>, Error> {
// 	let request = GetActiveOrders {
// 		pair: product.try_into()?,
// 		nonce: nonce(),
// 	};
// 	let request = request.authenticate(&self.credential);
// 	let response = self.client.send(&self.host, request)?;
//
// 	let mut orders = Vec::with_capacity(response.len());
// 	for (id, order) in response.into_iter() {
// 		let order = ccex::Order {
// 			id: None,
// 			server_id: Some(id),
// 			side: order.side.into(),
// 			product: order.pair.parse::<CurrencyPair>()?.try_into()?,
// 			status: ccex::OrderStatus::Open,
// 			instruction: ccex::OrderInstruction::Limit {
// 				price: d128::from_f64(order.rate).unwrap(),
// 				original_quantity: d128::zero(),
// 				remaining_quantity: d128::from_f64(order.amount).unwrap(),
// 				time_in_force: ccex::TimeInForce::GoodTillCancelled,
// 			}
// 		};
// 		orders.push(order);
// 	}
// 	Ok(orders)
// }

// pub type OrderId = String;
// impl<'a> NeedsAuthentication<&'a Credential> for GetActiveOrders {}
// impl<'a> RestResource for PrivateRequest<GetActiveOrders, &'a Credential> {
// 	type Response = HashMap<OrderId, Order>;
//
// 	fn method(&self) -> Method {
// 		Method::Post
// 	}
//
// 	fn path(&self) -> String {
// 		"/tapi".to_owned()
// 	}
//
// 	fn body(&self) -> Result<Option<Payload>, Error> {
// 		let body = QueryBuilder::with_capacity(3)
// 			.param("method", "ActiveOrders")
// 			.param("nonce", self.request.nonce.to_string())
// 			.param("pair", self.request.pair.to_string())
// 			.build();
//
// 		Ok(Some(Payload::Text(body.to_string())))
// 	}
//
// 	fn headers(&self) -> Result<Headers, Error> {
// 		private_headers(self, &self.credential)
// 	}
//
// 	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
// 		deserialize_private_response(response)
// 	}
// }

pub fn maker_fee() -> d128 {
    // 0.001 (0.01%)
    d128::new(1, 3)
}

pub fn taker_fee() -> d128 {
    // 0.0025 (0.025%)
    d128::new(25, 4)
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

fn nonce() -> u32 {
    // TODO: switch to a cached nonce at some point. this has the limitations
    // of 1) only allowing one request per millisecond and 2) expiring after
    // ~50 days
    let now = Utc::now();
    (now.timestamp() as u32 - 1_521_186_749u32) * 1000 + now.timestamp_subsec_millis()
}
