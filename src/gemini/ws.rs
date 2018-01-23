pub mod interface {
    use crate as ccex;
    use super::model;

    use url::Url;
    use serde_json;
    use gemini::{private_headers};
    use {Request, Method};
    use api;
    use gemini::Credential;

    impl From<ccex::Environment> for Url {
        fn from(env: ccex::Environment) -> Self {
            match env {
                ccex::Environment::Production => Url::parse("wss://api.gemini.com").unwrap(),
                ccex::Environment::Sandbox => Url::parse("wss://api.sandbox.gemini.com").unwrap(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct GetMarketStream {
        pub product: model::CurrencyPair,
    }

    impl api::WebsocketResource for GetMarketStream {
        type Message = model::market::Response;
        type Error = serde_json::Error;

        fn method(&self) -> api::Method {
            api::Method::Get
        }

        fn path(&self) -> String {
            format!("/v1/marketdata/{}", self.product)
        }

        fn serialize(message: Self::Message) -> Result<api::WebsocketMessage, Self::Error> {
            unimplemented!("There shouldn't be any messages sent over the market stream--it's receive only")
        }

        fn deserialize(message: api::WebsocketMessage) -> Result<Self::Message, Self::Error> {
            match message {
                api::WebsocketMessage::Text(message) => serde_json::from_str(&message),
                _ => unimplemented!(),
            }
        }

    }

    // pub fn market_stream<B, P>(base_address: B, product: P) -> Request
    //     where B: Into<Url>,
    //           P: Into<CurrencyPair> {
    //     const REQUEST: &'static str = "/v1/marketdata/";

    //     Request {
    //         address: base_address.into().join(REQUEST).unwrap().join(&product.into().to_string()).unwrap(),
    //         headers: None,
    //         method: Method::Get,
    //         payload: None,
    //     }
    // }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct GetOrderStream {
        pub nonce: i64,
        pub request: String,
    }

    impl GetOrderStream {
        pub fn new(nonce: i64) -> Self {
            let request = "/v1/order/events".to_owned();
            GetOrderStream {
                nonce,
                request,
            }
        }
    }

    impl<'a> api::NeedsAuthentication<&'a Credential> for GetOrderStream {}
    impl<'a> api::WebsocketResource for api::PrivateRequest<GetOrderStream, &'a Credential> {
        type Message = Vec<model::order::Response>;
        type Error = serde_json::Error;

        fn method(&self) -> api::Method {
            api::Method::Get
        }

        fn path(&self) -> String {
            self.request.request.clone()
        }

        fn headers(&self) -> api::Headers {
            private_headers(&self.request, &self.credential).unwrap()
        }

        fn serialize(message: Self::Message) -> Result<api::WebsocketMessage, Self::Error> {
            unimplemented!("There shouldn't be any messages sent over the order stream")
        }

        fn deserialize(message: api::WebsocketMessage) -> Result<Self::Message, Self::Error> {
            match message {
                api::WebsocketMessage::Text(message) => {
                    let deserialized: serde_json::Value = serde_json::from_str(&message)?;
                    match deserialized {
                        values @ serde_json::Value::Array(..) => serde_json::from_value(values),
                        value  @ serde_json::Value::Object(..) => {
                            serde_json::from_value(serde_json::Value::Array(vec![value]))
                        }
                        _ => unimplemented!(),
                    }
                }
                _ => unimplemented!(),
            }
        }
    }
}

pub mod model {
    use crate as ccex;
    use std::fmt;
    use std::fmt::Display;

    #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
    #[serde(rename_all = "lowercase")]
    pub enum CurrencyPair {
        BTCUSD,
        ETHUSD,
        ETHBTC,
    }

    impl Display for CurrencyPair {
        fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            match self {
                &CurrencyPair::BTCUSD => write!(f, "btcusd"),
                &CurrencyPair::ETHUSD => write!(f, "ethusd"),
                &CurrencyPair::ETHBTC => write!(f, "ethbtc"),
            }
        }
    }

    impl From<CurrencyPair> for ccex::CurrencyPair {
        fn from(currency_pair: CurrencyPair) -> Self {
            match currency_pair {
                CurrencyPair::BTCUSD => (ccex::Currency::BTC, ccex::Currency::USD),
                CurrencyPair::ETHUSD => (ccex::Currency::ETH, ccex::Currency::USD),
                CurrencyPair::ETHBTC => (ccex::Currency::ETH, ccex::Currency::BTC),
            }
        }
    }

    impl From<ccex::CurrencyPair> for CurrencyPair {
        fn from(currency_pair: ccex::CurrencyPair) -> Self {
            match currency_pair {
                (ccex::Currency::BTC, ccex::Currency::USD) => CurrencyPair::BTCUSD,
                (ccex::Currency::ETH, ccex::Currency::USD) => CurrencyPair::ETHUSD,
                (ccex::Currency::ETH, ccex::Currency::BTC) => CurrencyPair::ETHBTC,
                _ => panic!(),
            }
        }
    }

    pub mod market {
        use super::*;
        use crate as ccex;
        use decimal::d128;

        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
        #[serde(rename_all = "lowercase")]
        pub enum Side {
            Bid,
            Ask,
            Auction,
        }

        impl From<Side> for ccex::Side {
            fn from(side: Side) -> Self {
                match side {
                    Side::Bid => ccex::Side::Bid,
                    Side::Ask => ccex::Side::Ask,
                    _ => panic!(),
                }
            }
        }

        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
        #[serde(rename_all = "lowercase")]
        pub enum Reason {
            Initial,
            Place,
            Trade,
            Cancel,
        }

        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
        #[serde(rename_all = "lowercase")]
        pub enum AuctionIndicativeResult {
            Success,
            Failure,
        }

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        #[serde(rename_all = "lowercase", tag = "type")]
        pub enum Response {
            Heartbeat(Heartbeat),
            Update(Update),
        }

        impl From<(Response, CurrencyPair)> for ExchangeEvents {
            fn from((response, product): (Response, CurrencyPair)) -> Self {
                match response {
                    Response::Heartbeat(heartbeat) => ExchangeEvents(vec![ccex::ExchangeEvent::Heartbeat]),
                    Response::Update(update) => (update, product).into(),
                }
            }
        }

        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        pub struct Heartbeat {
            /// Zero-indexed monotonic increasing sequence number attached to each message sent - if
            /// there is a gap in this sequence, you have missed a message. If you choose to enable
            /// heartbeats, then `heartbeat` and `update` messages will share a single increasing
            /// sequence. See Sequence Numbers for more information.
            socket_sequence: i64,
        }

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        pub struct Update {
            /// Zero-indexed monotonic increasing sequence number attached to each message sent - if
            /// there is a gap in this sequence, you have missed a message. If you choose to enable
            /// heartbeats, then `heartbeat` and `update` messages will share a single increasing
            /// sequence. See Sequence Numbers for more information.
            pub socket_sequence: i64,

            /// A monotonically increasing sequence number indicating when this change occurred.
            /// These numbers are persistent and consistent between market data connections.
            #[serde(rename = "eventId")]
            pub event_id: i64,

            /// Either a change to the order book, or the indication that a trade has occurred.
            pub events: Vec<Event>,

            /// The timestamp in seconds for this group of events (included for compatibility
            /// reasons). We recommend using the `timestampms` field instead.
            pub timestamp: Option<i64>,

            /// The timestamp in milliseconds for this group of events.
            #[serde(rename = "timestampms")]
            pub timestamp_ms: Option<i64>,
        }

        pub struct ExchangeEvents(pub Vec<ccex::ExchangeEvent>);
        impl From<(Update, CurrencyPair)> for ExchangeEvents {
            fn from((mut update, product): (Update, CurrencyPair)) -> Self {
                let events = update.events.drain(..).map(|event| (event, product).into());

                ExchangeEvents(events.collect())
            }
        }

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        #[serde(rename_all = "snake_case", tag = "type")]
        pub enum Event {
            /// `AuctionOpen` advertises that an auction for this symbol is now open and accepting
            /// orders.
            AuctionOpen(AuctionOpen),
            /// `AuctionIndicative` advertises when an auction indicative price is published.
            AuctionIndicative(AuctionIndicative),
            AuctionResult(AuctionResult),
            Change(Change),
            Trade(Trade),
        }

        impl From<(Event, CurrencyPair)> for ccex::ExchangeEvent {
            fn from((event, product): (Event, CurrencyPair)) -> Self {
                match event {
                    Event::Change(change) => (change, product).into(),
                    Event::Trade(trade) => (trade, product).into(),
                    _ => unimplemented!(),
                }
            }
        }

        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        pub struct AuctionOpen {
            /// Time this auction opened
            pub auction_open_ms: i64,

            /// Time this auction will run
            pub auction_time_ms: i64,

            /// Time when the first indicative price will be published.
            pub first_indicative_ms: i64,

            /// Time when it will no longer be possible to cancel auction orders for this auction.
            pub last_cancel_time_ms: i64,
        }

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        pub struct AuctionIndicative {
            /// Unique event ID for this specific auction event.
            pub eid: i64,

            /// Indicates whether the auction would succeed if it were run at this point
            pub result: AuctionIndicativeResult,

            /// Time that this event occurred
            pub time_ms: i64,

            /// Highest bid price from the continuous order book at the time of the auction event,
            /// if available.
            pub highest_bid_price: d128,

            /// Lowest ask price from the continuous order book at the time of the auction event, if
            /// available.
            pub lowest_ask_price: d128,

            /// The `indicative_price` must be within plus or minus five percent of the collar price
            /// for `result` to be `success`.
            pub collar_price: d128,

            /// The price that this auction would take place at if it were run now. Zero if `result`
            /// is `failure`.
            pub indicative_price: d128,

            /// The quantity that would execute if the auction were run now.
            pub indicative_quantity: d128,
        }

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        pub struct AuctionResult {
            /// Unique event ID for this specific auction event.
            pub eid: i64,

            /// Indicates whether the auction would succeed if it were run at this point
            pub result: AuctionIndicativeResult,

            /// Time that this event occurred
            pub time_ms: i64,

            /// Highest bid price from the continuous order book at the time of the auction event,
            /// if available.
            pub highest_bid_price: d128,

            /// Lowest ask price from the continuous order book at the time of the auction event, if
            /// available.
            pub lowest_ask_price: d128,

            /// The `indicative_price` must be within plus or minus five percent of the collar price
            /// for `result` to be `success`.
            pub collar_price: d128,

            /// If `result` is `success`, the price at which orders were filled. Zero if `result` is
            /// `failure`.
            pub auction_price: d128,

            /// If `result` is `success`, the quantity that was filled. Zero if `result` is
            /// `failure`.
            pub auction_quantity: d128,
        }

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        pub struct Change {
            /// The price of this order book entry.
            pub price: d128,
            pub side: Side,
            pub reason: Reason,

            /// The quantity remaining at that price level after this change occurred. May be zero
            /// if all orders at this price level have been filled or canceled.
            pub remaining: d128,

            /// The quantity changed. May be negative, if an order is filled or canceled. For
            /// initial messages, delta will equal remaining.
            pub delta: d128,
        }

        impl From<(Change, CurrencyPair)> for ccex::ExchangeEvent {
            fn from((change, product): (Change, CurrencyPair)) -> Self {
                let offer = ccex::Offer::new(change.price, change.delta);
                let side = change.side.into();
                let product = product.into();
                if offer.supply.is_zero() {
                    ccex::ExchangeEvent::OrderbookOfferRemoved(product, side, offer)
                } else {
                    ccex::ExchangeEvent::OrderbookOfferUpdated(product, side, offer)
                }
            }
        }

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        pub struct Trade {
            /// The price this trade executed at.
            pub price: d128,

            /// The amount traded.
            pub amount: d128,

            /// The side of the book the maker of the trade placed their order on, of if the trade
            /// occurred in an auction. Either bid, ask, or auction.
            #[serde(rename = "makerSide")]
            pub maker_side: Side,

            #[serde(rename = "tid")] pub trade_id: i64,
        }

        impl From<Trade> for ccex::Trade {
            fn from(trade: Trade) -> Self {
                ccex::Trade {
                    price: trade.price,
                    quantity: trade.amount,
                    maker_side: trade.maker_side.into(),
                }
            }
        }

        impl From<(Trade, CurrencyPair)> for ccex::ExchangeEvent {
            fn from((trade, product): (Trade, CurrencyPair)) -> Self {
                ccex::ExchangeEvent::TradeExecuted(product.into(), trade.into())
            }
        }
    }

    pub mod order {
        use decimal::d128;
        use super::*;

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        #[serde(rename_all = "snake_case", tag = "type")]
        pub enum Response {
            /// Acknowledges your order events subscription and echoes back your parsed filters.
            SubscriptionAck(SubscriptionAck),

            /// Sent at five-second intervals to show that your WebSocket connection to Gemini is
            /// alive.
            Heartbeat {
                /// Gemini adds a timestamp so if you get disconnected, you may contact Gemini
                /// support with the timestamp of the last heartbeat you received.
                timestampms: i64,

                /// Gemini adds a monotonically incrementing sequence to make it easy to tell if
                /// you've missed a heartbeat. Not the same as `socket_sequence`!
                sequence: i64,

                /// Zero-indexed monotonic increasing sequence number attached to each message sent
                /// - if there is a gap in this sequence, you have missed a message. See Sequence
                /// Numbers for more information.
                socket_sequence: i64,

                /// Gemini adds a trace id to each WebSocket request that our networking team can
                /// use to trace your request in our logs.
                trace_id: String,
            },

            /// At the time you begin your subscription, you receive a list of your current active
            /// orders. Each active order will have the `initial` event type. You only see this
            /// event type at the beginning of each subscription.
            Initial(Order),

            /// Acknowledges that the exchange has received your order for initial processing. An
            /// order which cannot be accepted for initial processing receives a `rejected` event.
            Accepted(Order),

            /// When an order is rejected.
            Rejected(Order),

            /// Your order is now **visible** on the Gemini order book. Under certain conditions,
            /// when you place an order you will not receive a `booked` event. These include:
            ///
            /// * When placing a hidden order type, such as an auction-only order
            /// * When your order is completely filled after being accepted
            /// * When your order is accepted for initial processing but then immediately cancelled
            ///   because some condition cannot be fulfilled (for instance, if you submit a
            ///   maker-or-cancel order but your order would cross)
            Booked(Order),

            /// When an order is filled.
            Fill(Order),

            /// When an order is cancelled.
            Cancelled(Order),

            /// When your request to cancel an order cannot be fulfilled. Reasons this might happen
            /// include:
            ///
            /// * The order cannot be found
            /// * You're trying to cancel an auction-only order after the last simulation
            CancelRejected(Order),

            /// The last event in the order lifecycle: whether this order was completely filled or
            /// cancelled(Order), the consumer can use the `closed` event as a signal that the order
            /// is off the book on the Gemini side.
            Closed(Order),
        }

        impl From<Response> for ccex::ExchangeEvent {
            fn from(response: Response) -> Self {
                match response {
                    Response::Initial(order)        => ccex::ExchangeEvent::OrderAdded(order.into()),
                    Response::Booked(order)         => ccex::ExchangeEvent::OrderOpened(order.into()),
                    Response::Fill(order)           => ccex::ExchangeEvent::OrderFilled(order.into()),
                    Response::Cancelled(order)      => ccex::ExchangeEvent::OrderClosed(order.into()),
                    Response::Heartbeat{..}         => ccex::ExchangeEvent::Heartbeat,
                    Response::SubscriptionAck(ack)  => ccex::ExchangeEvent::Unimplemented(format!("{:?}", ack)),
                    r => panic!("Unhandled: {:?}", r),
                }
            }
        }

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct SubscriptionAck {
            /// The account id associated with the API session key you supplied in your
            /// `X-GEMINI-APIKEY` header
            pub account_id: i64,

            /// The id associated with this websocket subscription; the component after the last
            /// dash is a request trace id that will be echoed back in the heartbeat `traceId`
            /// field.
            pub subscription_id: String,

            /// An array of zero or more supported symbols. An empty array means your subscription
            /// is not filtered by symbol.
            pub symbol_filter: Vec<String>,

            /// An array of zero or more API session keys associated with your account. The string
            /// "UI" means you want to see orders placed by your website users. An empty array means
            /// you want to see all orders on your account, regardless of whether they were placed
            /// via the API or the website.
            pub api_session_filter: Vec<String>,

            /// An array of zero or more order event types. An empty array means your subscription
            /// is not filtered by event type.
            pub event_type_filter: Vec<String>,
        }

        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum Behavior {
            ImmediateOrCancel,
            MakerOrCancel,
        }

        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
        #[serde(rename_all = "lowercase")]
        pub enum Side {
            Buy,
            Sell,
        }

        impl From<Side> for ccex::Side {
            fn from(side: Side) -> Self {
                match side {
                    Side::Buy => ccex::Side::Bid,
                    Side::Sell => ccex::Side::Ask,
                }
            }
        }

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        pub struct Order {
            pub socket_sequence: i64,
            pub order_id: String,
            pub event_id: Option<String>,
            pub api_session: Option<String>,
            pub client_order_id: Option<String>,
            pub symbol: CurrencyPair,
            pub side: Side,
            pub behavior: Option<Behavior>,
            pub order_type: String,
            pub timestamp: String,
            pub timestampms: i64,
            pub is_live: bool,
            pub is_cancelled: bool,
            pub is_hidden: bool,
            pub avg_execution_price: Option<d128>,
            pub executed_amount: Option<d128>,
            pub remaining_amount: Option<d128>,
            pub original_amount: Option<d128>,
            pub price: Option<d128>,
            pub total_spend: Option<d128>,
            pub reason: Option<String>,
            pub fill: Option<Fill>,
            pub cancel_command_id: Option<String>,
        }

        impl From<Order> for ccex::Order {
            fn from(order: Order) -> Self {
                // FIXME: convert to new order type
                unimplemented!()
                // ccex::Order {
                //     id: order.order_id,
                //     product: order.symbol.into(),
                //     price: order.price.unwrap(),
                //     original_supply: order.original_amount.unwrap(),
                //     remaining_supply: order.remaining_amount.unwrap(),
                //     side: order.side.into(),
                // }
            }
        }

        #[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
        pub struct Fill {
            pub trade_id: String,
            pub liquidity: String,
            pub price: d128,
            pub amount: d128,
            pub fee: d128,
            pub fee_currency: String,
        }
    }
}
