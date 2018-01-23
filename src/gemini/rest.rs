
mod model {
    use crate as ccex;
    use decimal::d128;

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
                Product::BTCUSD => (ccex::Currency::BTC, ccex::Currency::USD),
                Product::ETHUSD => (ccex::Currency::ETH, ccex::Currency::USD),
                Product::ETHBTC => (ccex::Currency::ETH, ccex::Currency::BTC),
            }
        }
    }

    impl From<ccex::CurrencyPair> for Product {
        fn from(product: ccex::CurrencyPair) -> Self {
            match product {
                (ccex::Currency::BTC, ccex::Currency::USD) => Product::BTCUSD,
                (ccex::Currency::ETH, ccex::Currency::USD) => Product::ETHUSD,
                (ccex::Currency::ETH, ccex::Currency::BTC) => Product::ETHBTC,
                _ => panic!(),
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
}

mod request {
    use crate as ccex;
    use url::Url;
    use gemini;
    use decimal::d128;
    use {Request, Method};
    use api::{self, HttpResponse};
    use std::io;
    use serde_json;
    use super::model;
    use serde::de::DeserializeOwned;
    use serde::ser::Serialize;
    use std::fmt;
    use gemini::Credential;
    use failure::Error;

    pub trait GeminiRequest: fmt::Debug {
        type Response: DeserializeOwned;
        fn path(&self) -> String;
        fn method(&self) -> api::Method;
    }

    /// Generic implementation over any `PrivateRequest<T, _>` where `T: GeminiRequest`.
    /// This is useful because every gemini REST request is mostly similar
    impl<'a, T> api::RestResource for api::PrivateRequest<T, &'a Credential> where T: GeminiRequest + Serialize {
        type Response = T::Response;
        //type Error = Error;

        fn method(&self) -> api::Method {
            self.request.method()
        }

        fn path(&self) -> String {
            self.request.path()
        }

        fn headers(&self) -> Result<api::Headers, Error> {
            Ok(gemini::private_headers(&self.request, self.credential)?)
        }

        fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
            Ok(serde_json::from_slice(&response.body)?)
        }
    }

    /// Generic implementation for any public gemini REST request.
    /// This is equivalent to the impl for `PrivateRequest<GeminiRequest, _>` but for public requests
    impl<T> api::RestResource for T where T: GeminiRequest + Serialize {
        type Response = T::Response;
        //type Error = serde_json::Error;

        fn method(&self) -> api::Method {
            (self as &T).method()
        }

        fn path(&self) -> String {
            (self as &T).path()
        }

        fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
            Ok(serde_json::from_slice(&response.body)?)
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
        /// The literal string `"/v1/order/new"`
        pub request: String,
        /// Only `"exchange limit"` is supported at the moment
        pub ty: String,
        pub client_order_id: String,
        pub symbol: model::Product,
        pub amount: d128,
        pub price: d128,
        pub side: model::Side,
        pub options: Option<Vec<model::OrderExecutionOption>>,
    }

    impl PlaceOrder {
        pub fn new(client_order_id: String, symbol: model::Product, amount: d128, price: d128, side: model::Side, options: Option<Vec<model::OrderExecutionOption>>) -> Self {
            let request = "/v1/order/new".to_owned();
            let ty = "exchange limit".to_owned();
            PlaceOrder {
                request,
                ty,
                client_order_id,
                symbol,
                amount,
                price,
                side,
                options,
            }
        }
    }

    impl<'a> api::NeedsAuthentication<&'a Credential> for PlaceOrder {}
    impl GeminiRequest for PlaceOrder {
        type Response = model::OrderStatus;

        fn path(&self) -> String {
            format!("/v1/order/new")
        }

        fn method(&self) -> api::Method {
            api::Method::Post
        }
    }



    /// This will cancel an order. If the order is already canceled, the message will succeed but have no effect.
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct CancelOrder {
        nonce: i64,
        /// The order ID given by /order/new.
        order_id: i64,
        /// The literal string "/v1/order/cancel"
        request: String,
    }

    impl CancelOrder {
        pub fn new(nonce: i64, order_id: i64) -> Self {
            let request = "/v1/order/cancel".to_owned();
            CancelOrder {
                nonce,
                order_id,
                request,
            }
        }
    }

    impl<'a> api::NeedsAuthentication<&'a Credential> for CancelOrder {}
    impl GeminiRequest for CancelOrder {
        type Response = model::OrderStatus;

        fn method(&self) -> api::Method {
            api::Method::Post
        }

        fn path(&self) -> String {
            format!("/v1/order/cancel")
        }

    }


    #[derive(Debug)]
    pub struct GetBalances {
        /// The literal string "/v1/balances"
        pub request: String,
        pub nonce: i64,
    }

    // impl<'a> api::RestResource for Balances<'a> {
    //     type Response = Vec<Balance>;
    //     type Body = io::Empty;
    ////     type Error = serde_json::Error;

    //     fn method(&self) -> api::Method {
    //         api::Method::Get
    //     }

    //     fn path(&self) -> String {
    //         format!("/v1/balances")
    //     }

    //     fn headers(&self) -> Headers {
    //         #[derive(Serialize)]
    //         struct Payload {
    //             request: String,
    //             nonce: i64,
    //         }

    //         let payload = Payload {
    //             request: self.path(),
    //             none: self.nonce,
    //         };
    //         gemini::private_headers2(&payload, self.key, self.secret)
    //     }

    //     fn body(&self) -> Self::Body;

    //     fn parse<R>(&self, response: &mut R) -> Result<Self::Response, Error> where R: HttpResponse;
    // }

    #[derive(Debug, Serialize, Deserialize)]
    struct OrderCancellationRequest {
        pub request: String,
        pub nonce: i64,
        pub order_id: Option<i64>,
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
}

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


// }
