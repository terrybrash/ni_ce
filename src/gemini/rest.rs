use base64;
use decimal::d128;
use hex;
use reqwest;
use ring::{hmac, digest};
use serde_json;
use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Debug, Copy, Clone)]
pub enum Environment {
    Production,
    Sandbox,
}

impl Environment {
    fn base_address(&self) -> &'static str {
        match *self {
            Environment::Production => "https://api.gemini.com",
            Environment::Sandbox    => "https://api.sandbox.gemini.com",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename="lowercase")]
pub enum Product {
    BTCUSD,
    ETHUSD,
    ETHBTC,
}

#[derive(Debug, Copy, Clone, Deserialize)]
pub enum Currency {
    BTC,
    USD,
    ETH,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Serialize, Deserialize)]
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
    /// It will fill whatever part of the order it can immediately, then cancel any remaining 
    /// amount so that no part of the order is added to the order book.
    /// 
    /// If the order doesn't fully fill immediately, the response back from the API will indicate 
    /// that the order has already been canceled (`"is_cancelled"`: true in JSON).
    ImmediateOrCancel,

    /// This order will be added to the auction-only book for the next auction for this symbol.
    AuctionOnly,
}

#[derive(Debug, Deserialize)]
pub struct OrderStatus {
    /// Description of the order: 
    /// * `"exchange limit"`
    /// * `"auction-only exchange limit"`
    /// * `"market buy"`
    /// * `"auction-only market buy"`
    /// * `"market sell"`
    /// * `"auction-only market sell"`
    #[serde(rename="type")]
    pub ty: String,
    
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

    /// The timestamp the order was submitted. Note that for compatibility reasons, this is 
    /// returned as a string. It's recommended to use the `timestampms` field instead.
    pub timestamp: String,

    /// The timestamp the order was submitted in milliseconds.
    pub timestampms: i64,

    /// Will always be false.
    pub was_forced: Option<bool>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
struct BalancesRequest {
    pub request: String,
    pub nonce: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct OrderCancellationRequest {
    pub request: String,
    pub nonce: i64,
    pub order_id: Option<i64>,
}

/// Only limit orders are supported through the API at present.

/// If you wish orders to be automatically cancelled when your session ends, see the require 
/// heartbeat section, or manually send the cancel all session orders message.
///
/// [Documentation](https://docs.gemini.com/rest-api/#new-order)
#[derive(Debug, Serialize)]
struct OrderPlacementRequest {
    /// The literal string `"/v1/order/new"`
    pub request: String,

    /// The order type. Only `"exchange limit"` supported through this API
    #[serde(rename="type")]
    pub ty: String,

    pub nonce: i64,
    pub client_order_id: String,
    pub symbol: Product,
    pub amount: d128,
    pub price: d128,
    pub side: Side,
    pub options: Option<Vec<OrderExecutionOption>>,
}

pub fn place_order(env: &Environment, client: &mut reqwest::Client, api_key: &str, api_secret: &str, nonce: i64, client_order_id: String, symbol: Product, amount: d128, price: d128, side: Side) -> OrderStatus {
    const REQUEST: &'static str = "/v1/order/new";
    let order_placement = OrderPlacementRequest {
        request: REQUEST.to_owned(),
        ty: "exchange limit".to_owned(),
        nonce: nonce,
        client_order_id: client_order_id,
        symbol: symbol,
        amount: amount,
        price: price,
        side: side,
        options: None,
    };

    private_request(env, client, api_key, api_secret, reqwest::Method::Post, REQUEST, &order_placement)
}

/// This will cancel an order. 
/// 
/// If the order is already canceled, the message will succeed but have no effect.
///
/// The API key you use to access this endpoint must have the **Trader** role assigned. 
/// See [Roles](https://docs.gemini.com/rest-api/#roles) for more information.
pub fn cancel_order(env: &Environment, client: &mut reqwest::Client, api_key: &str, api_secret: &str, nonce: i64, order_id: i64) -> OrderStatus {
    const REQUEST: &'static str = "/v1/order/cancel";
    let order_cancellation = OrderCancellationRequest {
        nonce: nonce,
        request: REQUEST.to_owned(),
        order_id: Some(order_id),
    };

    private_request(env, client, api_key, api_secret, reqwest::Method::Post, REQUEST, &order_cancellation)
}

/// This will cancel all orders opened by this session.
/// 
/// This will have the same effect as heartbeat expiration if "Require Heartbeat" is selected 
/// for the session.
///
/// The API key you use to access this endpoint must have the **Trader** role assigned. 
/// See [Roles](https://docs.gemini.com/rest-api/#roles) for more information.
pub fn cancel_session_orders(env: &Environment, client: &mut reqwest::Client, api_key: &str, api_secret: &str, nonce: i64) {
    const REQUEST: &'static str = "/v1/order/cancel/session";
    let order_cancellation = OrderCancellationRequest {
        nonce: nonce,
        request: REQUEST.to_owned(),
        order_id: None,
    };

    private_request(env, client, api_key, api_secret, reqwest::Method::Post, REQUEST, &order_cancellation)
}

/// This will cancel all outstanding orders created by all sessions owned by this account, 
/// including interactive orders placed through the UI.
///
/// The API key you use to access this endpoint must have the **Trader** role assigned. 
/// See [Roles](https://docs.gemini.com/rest-api/#roles) for more information.
pub fn cancel_all_orders(env: &Environment, client: &mut reqwest::Client, api_key: &str, api_secret: &str, nonce: i64) {
    const REQUEST: &'static str = "/v1/order/cancel/all";
    let order_cancellation = OrderCancellationRequest {
        nonce: nonce,
        request: REQUEST.to_owned(),
        order_id: None,
    };

    private_request(env, client, api_key, api_secret, reqwest::Method::Post, REQUEST, &order_cancellation)
}

/// This will show the available balances in the supported currencies
///
/// The API key you use to access this endpoint must have the **Trader** or **Fund Manager** role assigned. 
/// See [Roles](https://docs.gemini.com/rest-api/#roles) for more information.
pub fn available_balances(env: &Environment, client: &mut reqwest::Client, api_key: &str, api_secret: &str, nonce: i64) -> Vec<Balance> {
    const REQUEST: &'static str = "/v1/balances";
    let balances_request = BalancesRequest {
        request: REQUEST.to_owned(),
        nonce: nonce,
    };

    private_request(env, client, api_key, api_secret, reqwest::Method::Post, REQUEST, &balances_request)
}

header! {(XGeminiApikey, "X-GEMINI-APIKEY") => [String]}
header! {(XGeminiPayload, "X-GEMINI-PAYLOAD") => [String]}
header! {(XGeminiSignature, "X-GEMINI-SIGNATURE") => [String]}

fn private_request<S, D>(env: &Environment, client: &mut reqwest::Client, api_key: &str, api_secret: &str, method: reqwest::Method, request: &str, payload: &S) -> D
    where S: Serialize,
          D: DeserializeOwned {
    let request_url = format!("{}{}", env.base_address(), request);

    let payload = serde_json::to_string(payload).unwrap();
    let payload = base64::encode(&payload);

    let signing_key = hmac::SigningKey::new(&digest::SHA384, api_secret.as_bytes());
    let signature = hex::encode(hmac::sign(&signing_key, payload.as_bytes()));

    let mut request_builder = match method {
        reqwest::Method::Get => client.get(&request_url),
        reqwest::Method::Post => client.post(&request_url),
        _ => unimplemented!(),
    };

    let response = request_builder
        .header(XGeminiApikey(api_key.to_owned()))
        .header(XGeminiPayload(payload))
        .header(XGeminiSignature(signature))
        .send().unwrap();
    
    serde_json::from_reader(response).unwrap()
}