use api::{
    self,
    HttpResponse,
    Header,
    Headers,
    NeedsAuthentication,
    RestResource,
    PrivateRequest,
    Method,
    HttpClient,
};
use crate as ccex;
use rust_decimal::Decimal as d128;
use failure::{Error, ResultExt};
use gemini::Credential;
use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use serde_json;
use std::fmt;
use std::io;
use url::Url;
use gemini::private_headers;
use std::convert::TryFrom;
use Exchange;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename="lowercase")]
pub enum Product {
    BTCUSD,
    ETHUSD,
    ETHBTC,
}

impl From<Product> for ccex::CurrencyPair {
    fn from(product: Product) -> Self {
        match product {
            Product::BTCUSD => ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::USD),
            Product::ETHUSD => ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::USD),
            Product::ETHBTC => ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::BTC),
        }
    }
}

impl TryFrom<ccex::CurrencyPair> for Product {
    type Error = Error;
    fn try_from(product: ccex::CurrencyPair) -> Result<Self, Self::Error> {
        match product {
            ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::USD) => Ok(Product::BTCUSD),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::USD) => Ok(Product::ETHUSD),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::BTC) => Ok(Product::ETHBTC),
            product => Err(format_err!("{:?} isn't supported", product)),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum Currency {
    BTC,
    USD,
    ETH,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all="lowercase")]
pub enum Side {
    Buy,
    Sell,
}

impl From<Side> for ccex::Side {
    fn from(side: Side) -> Self {
        match side {
            Side::Buy   => ccex::Side::Bid,
            Side::Sell  => ccex::Side::Ask,
        }
    }
}

impl From<ccex::Side> for Side {
    fn from(side: ccex::Side) -> Self {
        match side {
            ccex::Side::Bid => Side::Buy,
            ccex::Side::Ask => Side::Sell,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="kebab-case")]
pub enum OrderExecutionOption {
    /// This order will only add liquidity to the order book.
    ///
    /// If any part of the order could be filled immediately, the whole order will instead be
    /// canceled before any execution occurs.
    ///
    /// If that happens, the response back from the API will indicate that the order has already
    /// been canceled (`"is_cancelled"`: true in JSON).
    ///
    /// Note: some other exchanges call this option *"post-only"*.
    MakerOrCancel,

    /// This order will only remove liquidity from the order book.
    ///
    /// It will fill whatever part of the order it can immediately, then cancel any remaining amount
    /// so that no part of the order is added to the order book.
    /// 
    /// If the order doesn't fully fill immediately, the response back from the API will indicate
    /// that the order has already been canceled (`"is_cancelled"`: true in JSON).
    ImmediateOrCancel,

    /// This order will be added to the auction-only book for the next auction for this symbol.
    AuctionOnly,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderStatus {
    /// Description of the order: 
    /// * `"exchange limit"`
    /// * `"auction-only exchange limit"`
    /// * `"market buy"`
    /// * `"auction-only market buy"`
    /// * `"market sell"`
    /// * `"auction-only market sell"`
    #[serde(rename="type")]
    pub _type: String,
    
    /// The average price at which this order as been executed so far. 0 if the order has not been
    /// executed at all.
    pub avg_execution_price: d128,
    
    /// An optional [client-specified order id](https://docs.gemini.com/rest-api/#client-order-id)
    pub client_order_id: Option<String>,

    /// Will always be `"gemini"`
    pub exchange: String,

    /// The amount of the order that has been filled.
    pub executed_amount: d128,

    /// `true` if the order has been canceled.
    pub is_cancelled: bool,

    /// TODO: document
    pub is_hidden: Option<bool>,
    
    /// `true` if the order is active on the book (has remaining quantity and has not been canceled)
    pub is_live: bool,
    
    /// An array containing at most one supported order execution option
    pub options: Option<Vec<OrderExecutionOption>>,

    /// The order id
    pub order_id: i64,

    /// The originally submitted amount of the order.
    pub original_amount: d128,

    /// The price the order was issued at
    pub price: d128,

    /// The amount of the order that has not been filled.
    pub remaining_amount: d128,
    pub side: Side,
    
    /// The [symbol](https://docs.gemini.com/rest-api/#symbols-and-minimums) of the order
    pub symbol: Product,

    /// The timestamp the order was submitted. Note that for compatibility reasons, this is returned
    /// as a string. It's recommended to use the `timestampms` field instead.
    pub timestamp: String,

    /// The timestamp the order was submitted in milliseconds.
    pub timestampms: i64,

    /// Will always be false.
    pub was_forced: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all="camelCase")]
pub struct Balance {
    pub currency: Currency,

    /// The current balance
    pub amount: d128,

    /// The amount that is available to trade
    pub available: d128,

    /// The amount that is available to withdraw
    pub available_for_withdrawal: d128,
}

fn deserialize_response<D>(response: &HttpResponse) -> Result<D, Error> 
where D: DeserializeOwned {
    match response.body {
        Some(api::Payload::Text(ref body)) => {
            let response = serde_json::from_str(body)
                .map_err(|e| format_err!("{}", e))
                .with_context(|_| format_err!("failed to deserialize \"{}\"", body))?;
            Ok(response)
        }
        Some(api::Payload::Binary(ref body)) => {
            let response = serde_json::from_slice(body)
                .map_err(|e| format_err!("{}", e))
                .with_context(|_| format_err!("failed to deserialize <binary>"))?;
            Ok(response)
        }
        None => {
            Err(format_err!("the body is empty"))?
        }
    }
}

/// Only limit orders are supported through the API at present.
///
/// If you wish orders to be automatically cancelled when your session ends, see the require
/// heartbeat section, or manually send the cancel all session orders message.
///
/// [Documentation](https://docs.gemini.com/rest-api/#new-order)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceOrder {
    pub client_order_id: String,
    pub symbol: Product,
    pub amount: d128,
    pub price: d128,
    pub side: Side,
    pub options: Option<Vec<OrderExecutionOption>>,
}

impl<'a> NeedsAuthentication<&'a Credential> for PlaceOrder {}
impl<'a> RestResource for PrivateRequest<PlaceOrder, &'a Credential> {
    type Response = OrderStatus;

    fn path(&self) -> String {
        "/v1/order/new".to_string()
    }

    fn method(&self) -> Method {
        Method::Post
    }

    fn headers(&self) -> Result<Headers, Error> {
        #[derive(Serialize)]
        struct Payload<'a> {
            request: &'static str,
            #[serde(rename="type")]
            _type: &'static str,
            client_order_id: &'a str,
            symbol: Product,
            amount: &'a d128,
            price: &'a d128,
            side: Side,
            options: Option<&'a [OrderExecutionOption]>,
        }

        let payload = Payload {
            request: "/v1/order/new",
            _type: "exchange limit",
            client_order_id: &self.request.client_order_id,
            symbol: self.request.symbol,
            amount: &self.request.amount,
            price: &self.request.price,
            side: self.request.side,
            options: self.request.options.as_ref().map(|options| options.as_slice()),
        };

        private_headers(&payload, &self.credential)
    }

    fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
        deserialize_response(response)
    }
}

/// This will cancel an order. If the order is already canceled, the message will succeed but have no effect.
#[derive(Clone, Debug, Serialize)]
pub struct CancelOrder {
    nonce: i64,
    /// The order ID given by /order/new.
    order_id: i64,
}

impl<'a> api::NeedsAuthentication<&'a Credential> for CancelOrder {}
impl<'a> RestResource for PrivateRequest<CancelOrder, &'a Credential> {
    type Response = OrderStatus;

    fn method(&self) -> Method {
        Method::Post
    }

    fn path(&self) -> String {
        "/v1/order/cancel".to_string()
    }

    fn headers(&self) -> Result<Headers, Error> {
        #[derive(Serialize)]
        struct Payload {
            request: &'static str,
            nonce: i64,
            order_id: i64,
        }

        let payload = Payload {
            request: "/v1/order/cancel",
            nonce: self.request.nonce,
            order_id: self.request.order_id,
        };

        private_headers(&payload, &self.credential)
    }

    fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
        deserialize_response(response)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GetBalances {
    pub nonce: i64,
}

impl<'a> NeedsAuthentication<&'a Credential> for GetBalances {}
impl<'a> RestResource for PrivateRequest<GetBalances, &'a Credential> {
    type Response = Vec<Balance>;

    fn method(&self) -> Method {
        Method::Get
    }

    fn path(&self) -> String {
        "/v1/balances".to_string()
    }

    fn headers(&self) -> Result<Headers, Error> {
        #[derive(Serialize)]
        struct Payload {
            request: &'static str,
            nonce: i64,
        }

        let payload = Payload {
            request: "/v1/balances",
            nonce: self.request.nonce,
        };

        private_headers(&payload, &self.credential)
    }

    fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
        deserialize_response(response)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OrderCancellationRequest {
    pub request: String,
    pub nonce: i64,
    pub order_id: Option<i64>,
}



pub struct Gemini {
    pub credential: Credential,
}

impl<Client> Exchange<Client> for Gemini 
where Client: HttpClient {
    fn name(&self) -> &'static str {
        "Gemini"
    }

    fn orderbook_cooldown(&self) -> Duration {
        Duration::from_millis(500)
    }
}





























// let response = cancel_order(nonce, order_id)
//     .authenticate((key, secret))
//     .send(client, address);

// let request = cancel_order(nonce, order_id).authenticate((key, secret));
// let response = client.send(address, request);

// pub fn cancel_order(nonce: i64, order_id: i64) -> IncompletePrivateRequest<CancelOrder> {
//     IncompletePrivateRequest::new(CancelOrder {
//         request: "/v1/order/cancel",
//         nonce: nonce,
//         order_id: i64,
//     })
// }



// impl api::RestResource for PrivateRequest<CancelOrder, (String, String)> {
//     type Response = Vec<Balance>;
//     type Body = io::Empty;
////     type Error = serde_json::Error;

//     fn method(&self) -> api::Method {
//         api::Method::Get
//     }

//     fn path(&self) -> String {
//         format!("/v1/order/cancel")
//     }

//     fn headers(&self) -> Headers {
//         let (ref key, ref secret) = self.credential;
//         gemini::private_headers2(&self.request, key, secret)
//     }

//     fn body(&self) -> Self::Body {
//         io::empty()
//     }

//     fn parse<R>(&self, response: &mut R) -> Result<Self::Response, Error> where R: HttpResponse {
//         serde_json::from_reader(response.body())
//     }
// }

// impl api::Request for AuthenticatedPrivateRequest<CancelOrder, (String, String)> {
//     type Error: serde_json::Error;
//     type Response: model::OrderStatus;

//     pub fn headers()
// }

// pub struct CancelOrder {
//     pub request: String,
//     pub nonce: i64,
//     pub order_id: i64,
// }

// pub enum Request {
//     Private(T, C),
//     Public(T),
// }


// /// This will cancel an order. 
// /// 
// /// If the order is already canceled, the message will succeed but have no effect.
// ///
// /// The API key you use to access this endpoint must have the **Trader** role assigned. See
// /// [Roles](https://docs.gemini.com/rest-api/#roles) for more information.
// pub fn cancel_order<B>(base_address: B, key: &str, secret: &str, nonce: i64, order_id: i64) -> Request where B: Into<Url> {
//     const REQUEST: &'static str = "/v1/order/cancel";
//     let order_cancellation = OrderCancellationRequest {
//         nonce: nonce,
//         request: REQUEST.to_owned(),
//         order_id: Some(order_id),
//     };

//     Request {
//         address:    base_address.into().join(REQUEST).unwrap(),
//         headers:    Some(gemini::private_headers(&order_cancellation, key, secret)),
//         method:     Method::Post,
//         payload:    None,
//     }
// }

// /// This will cancel all orders opened by this session.
// /// 
// /// This will have the same effect as heartbeat expiration if "Require Heartbeat" is selected for
// /// the session.
// ///
// /// The API key you use to access this endpoint must have the **Trader** role assigned. See
// /// [Roles](https://docs.gemini.com/rest-api/#roles) for more information.
// pub fn cancel_session_orders<B>(base_address: B, key: &str, secret: &str, nonce: i64) -> Request where B: Into<Url> {
//     const REQUEST: &'static str = "/v1/order/cancel/session";
//     let order_cancellation = OrderCancellationRequest {
//         nonce: nonce,
//         request: REQUEST.to_owned(),
//         order_id: None,
//     };

//     Request {
//         address:    base_address.into().join(REQUEST).unwrap(),
//         headers:    Some(gemini::private_headers(&order_cancellation, key, secret)),
//         method:     Method::Post,
//         payload:    None,
//     }
// }

// /// This will cancel all outstanding orders created by all sessions owned by this account, including
// /// interactive orders placed through the UI.
// ///
// /// The API key you use to access this endpoint must have the **Trader** role assigned. See
// /// [Roles](https://docs.gemini.com/rest-api/#roles) for more information.
// pub fn cancel_all_orders<B>(base_address: B, key: &str, secret: &str, nonce: i64) -> Request where B: Into<Url> {
//     const REQUEST: &'static str = "/v1/order/cancel/all";
//     let order_cancellation = OrderCancellationRequest {
//         nonce: nonce,
//         request: REQUEST.to_owned(),
//         order_id: None,
//     };

//     Request {
//         address:    base_address.into().join(REQUEST).unwrap(),
//         headers:    Some(gemini::private_headers(&order_cancellation, key, secret)),
//         method:     Method::Post,
//         payload:    None,
//     }
// }

// /// This will show the available balances in the supported currencies
// ///
// /// The API key you use to access this endpoint must have the **Trader** or **Fund Manager** role
// /// assigned. See [Roles](https://docs.gemini.com/rest-api/#roles) for more information.
// pub fn available_balances<B>(base_address: B, key: &str, secret: &str, nonce: i64) -> Request where B: Into<Url> {
//     const REQUEST: &'static str = "/v1/balances";
//     let balances_request = BalancesRequest {
//         request: REQUEST.to_owned(),
//         nonce: nonce,
//     };

//     Request {
//         address:    base_address.into().join(REQUEST).unwrap(),
//         headers:    Some(gemini::private_headers(&balances_request, key, secret)),
//         method:     Method::Post,
//         payload:    None,
//     }
// }

// pub fn place_order<B>(base_address: B, key: &str, secret: &str, nonce: i64, order: ccex::NewOrder) -> Request
//     where B: Into<Url> {
//     const REQUEST: &'static str = "/v1/order/new";
//     let order_placement = OrderPlacementRequest {
//         request:            REQUEST.to_owned(),
//         ty:                 "exchange limit".to_owned(),
//         nonce:              nonce,
//         client_order_id:    order.id,
//         symbol:             order.product.into(),
//         amount:             order.supply,
//         price:              order.price,
//         side:               order.side.into(),
//         options:            None,
//     };

//     Request {
//         address:    base_address.into().join(REQUEST).unwrap(),
//         headers:    Some(gemini::private_headers(&order_placement, key, secret)),
//         method:     Method::Post,
//         payload:    None,
//     }
// }


// pub mod interface {
//     pub fn url_from_environment(env: ccex::Environment) -> Url {
//         match env {
//             ccex::Environment::Production => Url::parse("https://api.gemini.com").unwrap(),
//             ccex::Environment::Sandbox    => Url::parse("https://api.sandbox.gemini.com").unwrap(),
//         }
//     }
