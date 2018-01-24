use api::{self, HttpResponse};
use base64;
use chrono::DateTime;
use chrono::Utc;
use decimal::d128;
use hmac::{Hmac, Mac};
use serde_json;
use sha2;
use std::io::{self, Read, Cursor};
use gdax::{Credential, private_headers, CurrencyPair, Currency, Side};
use crate as ccex;
use std::convert::TryFrom;

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

impl From<(TimeInForce, Option<CancelAfter>, Option<String>)> for ccex::TimeInForce {
    fn from(time_in_force: (TimeInForce, Option<CancelAfter>, Option<String>)) -> Self {
        match time_in_force {
            (TimeInForce::GoodTillCanceled,     _,                  None) => ccex::TimeInForce::GoodTillCancelled,
            (TimeInForce::FillOrKill,           _,                  None) => ccex::TimeInForce::FillOrKill,
            (TimeInForce::ImmediateOrCancel,    _,                  None) => ccex::TimeInForce::ImmediateOrCancel,
            (TimeInForce::GoodTillTime,         None,               Some(expire_time)) => ccex::TimeInForce::GoodTillCancelled, // FIXME: this should be manually parsed into DateTime<UTC>, expire_time isn't a normal DateTime<UTC> string 
            (TimeInForce::GoodTillTime,         Some(cancel_after), None) => {
                match cancel_after {
                    CancelAfter::Min => ccex::TimeInForce::GoodForMin,
                    CancelAfter::Hour => ccex::TimeInForce::GoodForHour,
                    CancelAfter::Day => ccex::TimeInForce::GoodForDay,
                }
            }
            time_in_force => unimplemented!("unexpected conversion from {:?}", time_in_force)
        }
    }
}


#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum Reason {
    Filled,
    Canceled,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum OrderStatus {
    Done,
    Settled,
    Open,
    Pending,
    Active,
    Rejected,
}

impl From<(OrderStatus, Option<Reason>)> for ccex::OrderStatus {
    fn from(status: (OrderStatus, Option<Reason>)) -> Self {
        match status {
            (OrderStatus::Pending, _)                   => ccex::OrderStatus::Pending,
            (OrderStatus::Done, _)                      => ccex::OrderStatus::Closed("no reason given".to_owned()),
            (OrderStatus::Done, Some(Reason::Filled))   => ccex::OrderStatus::Filled,
            (OrderStatus::Done, Some(Reason::Canceled)) => ccex::OrderStatus::Closed("Cancelled".to_owned()),
            (OrderStatus::Open, _)                      => ccex::OrderStatus::Open,
            (OrderStatus::Rejected, _)                  => ccex::OrderStatus::Rejected("no reason given".to_owned()),
            status                                      => unimplemented!("{:?}", status)
        }
    }
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
pub enum PlaceOrder {
    Limit(PlaceLimitOrder),
    Market(PlaceMarketOrder),
    Stop(PlaceStopOrder),
}

impl From<ccex::NewOrder> for PlaceOrder {
    fn from(order: ccex::NewOrder) -> Self {
        match order.instruction {
            ccex::NewOrderInstruction::Limit {price, quantity, time_in_force} => {
                let (time_in_force, cancel_after) = match time_in_force {
                    ccex::TimeInForce::GoodTillCancelled    => (TimeInForce::GoodTillCanceled, None),
                    ccex::TimeInForce::FillOrKill           => (TimeInForce::FillOrKill, None),
                    ccex::TimeInForce::ImmediateOrCancel    => (TimeInForce::ImmediateOrCancel, None),
                    ccex::TimeInForce::GoodForDay           => (TimeInForce::GoodTillTime, Some(CancelAfter::Day)),
                    ccex::TimeInForce::GoodForHour          => (TimeInForce::GoodTillTime, Some(CancelAfter::Hour)),
                    ccex::TimeInForce::GoodForMin           => (TimeInForce::GoodTillTime, Some(CancelAfter::Min)),
                    _ => unimplemented!(),
                };

                let place_limit_order = PlaceLimitOrder {
                    client_oid: order.id.to_string(),
                    side: order.side.into(),
                    product: order.product.into(),
                    stp: None,

                    price: price,
                    size: quantity,
                    time_in_force: Some(time_in_force),
                    cancel_after: cancel_after,
                };

                PlaceOrder::Limit(place_limit_order)
            }
            _ => unimplemented!(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceLimitOrder {
    /// Order ID selected by you to identify your order
    pub client_oid: String,
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
pub struct PlaceMarketOrder {
    /// Order ID selected by you to identify your order
    pub client_oid: String,
    pub side: Side,
    #[serde(rename="product_id")]
    pub product: CurrencyPair,
    pub stp: Option<SelfTrade>,

    pub size: Option<d128>,
    pub funds: Option<d128>,
}

/// One of `size` or `funds` is required
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceStopOrder {
    /// Order ID selected by you to identify your order
    pub client_oid: String,
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

impl TryFrom<Order> for ccex::Order {
    type Error = String;
    fn try_from(order: Order) -> Result<Self, Self::Error> {
        match order {
            Order::Limit(order) => {
                Ok(ccex::Order {
                    id: None,
                    server_id: Some(order.id.parse().unwrap()),
                    side: order.side.into(),
                    product: order.product.into(),
                    status: (order.status, order.done_reason).into(),
                    instruction: ccex::OrderInstruction::Limit {
                        price: order.price,
                        remaining_quantity: order.size - order.executed_value,
                        original_quantity:  order.size,
                        time_in_force:      (order.time_in_force, order.cancel_after, order.expire_time).into(),
                    }
                })
            },
            Order::Market(order) => Err(format!("market orders aren't supported")),
            Order::Stop(order) => Err(format!("stop orders aren't supported")),
        }
    }
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
    pub done_at: Option<DateTime<Utc>>,
    pub done_reason: Option<Reason>,
    // pub expire_time: Option<DateTime<Utc>>,
    pub expire_time: Option<String>,

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
    pub done_at: Option<DateTime<Utc>>,
    pub done_reason: Option<Reason>,
    // pub expire_time: Option<DateTime<Utc>>,
    pub expire_time: Option<String>,

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
    pub done_at: Option<DateTime<Utc>>,
    pub done_reason: Option<Reason>,
    // pub expire_time: Option<DateTime<Utc>>,
    pub expire_time: Option<String>,

    pub price: d128,
    pub size: Option<d128>,
    pub funds: Option<d128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub currency: Currency,
    pub balance: d128,
    pub available: d128,
    pub hold: d128,
    pub profile_id: String,
}
    
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ErrorMessage {
    pub message: String,
}

// #[derive(Fail, Debug, Clone, Serialize, Deserialize)]
// #[fail(display = "the server returned {}: {}", code, message)]
// pub struct GdaxError {
//     pub code: u16,
//     pub message: String,
// }

// #[derive(Debug, Fail)]
// pub enum Error {
//     SerdeError(serde_json::Error),
//     #[fail(display = "the server returned {}: {}", code, message)]
//     BadRequest {
//         code: u16,
//         message: String,
//     }
// }
use failure::Error;

impl<'a> api::NeedsAuthentication<&'a Credential> for PlaceOrder {}
impl<'a> api::RestResource for api::PrivateRequest<PlaceOrder, &'a Credential> {
    type Response = Order;
    // type Error = Error;

    fn method(&self) -> api::Method {
        api::Method::Post
    }

    fn path(&self) -> String {
        format!("/orders")
    }

    fn body(&self) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(&self.request)?)
    }

    fn headers(&self) -> Result<api::Headers, Error> {
        Ok(private_headers(self, &self.credential)?)
    }

    fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
        if response.status == 200 {
            Ok(serde_json::from_slice(&response.body)?)
        } else {
            let error: ErrorMessage = serde_json::from_slice(&response.body)?;
            Err(format_err!("the server returned {}: {}", response.status, error.message))
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct GetOrders;
impl<'a> api::NeedsAuthentication<&'a Credential> for GetOrders {}
impl<'a> api::RestResource for api::PrivateRequest<GetOrders, &'a Credential> {
    type Response = Vec<Order>;
    // type Error = Error;

    fn method(&self) -> api::Method {
        api::Method::Get
    }

    fn path(&self) -> String {
        format!("/orders")
    }

    fn query(&self) -> api::Query {
        vec![
            ("status".to_owned(), "all".to_owned()),
        ]
    }

    fn headers(&self) -> Result<api::Headers, Error> {
        Ok(private_headers(self, &self.credential)?)
    }

    fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
        println!("{}", String::from_utf8(response.body.clone()).unwrap());
        if response.status == 200 {
            Ok(serde_json::from_slice(&response.body)?)
        } else {
            let error: ErrorMessage = serde_json::from_slice(&response.body)?;
            Err(format_err!("the server returned {}: {}", response.status, error.message))
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct GetAccounts;
impl<'a> api::NeedsAuthentication<&'a Credential> for GetAccounts {}
impl<'a> api::RestResource for api::PrivateRequest<GetAccounts, &'a Credential> {
    type Response = Vec<Account>;
    // type Error = Error;

    fn method(&self) -> api::Method {
        api::Method::Get
    }

    fn path(&self) -> String {
        format!("/accounts")
    }

    fn headers(&self) -> Result<api::Headers, Error> {
        Ok(private_headers(self, &self.credential)?)
    }

    fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
        if response.status == 200 {
            Ok(serde_json::from_slice(&response.body)?)
        } else {
            let error: ErrorMessage = serde_json::from_slice(&response.body)?;
            Err(format_err!("the server returned {}: {}", response.status, error.message))
        }
    }
}

#[derive(Debug)]
pub struct CancelOrder<'a> {
    pub order_id: &'a str,
}
impl<'a, 'b> api::NeedsAuthentication<&'a Credential> for CancelOrder<'b> {}
impl<'a, 'b> api::RestResource for api::PrivateRequest<CancelOrder<'b>, &'a Credential> {
    type Response = Order;
    // type Error = Error;

    fn method(&self) -> api::Method {
        api::Method::Delete
    }

    fn path(&self) -> String {
        format!("/orders/{}", self.request.order_id)
    }

    fn headers(&self) -> Result<api::Headers, Error> {
        Ok(private_headers(self, &self.credential)?)
    }

    fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
        if response.status == 200 {
            Ok(serde_json::from_slice(&response.body)?)
        } else {
            let error: ErrorMessage = serde_json::from_slice(&response.body)?;
            Err(format_err!("the server returned {}: {}", response.status, error.message))
        }
    }
}
            // let error = GdaxError {
            //     code: response.status,
            //     message: error_message.message,
            // };
            // Err(error)?
