use decimal::d128;
use std::fmt;
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub type CurrencyPair = (Currency, Currency);
pub type ID = i64;

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum Currency {
    BTC,
    ETH,
    USDT,
    USD,
    BCH,
    LTC,
    GBP,
    EUR,
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum Environment {
    Production,
    Sandbox,
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub currency: Currency,
    pub balance: d128,
}

#[derive(Debug, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub maker_side: Side,
    pub price: d128,
    pub quantity: d128,
}

#[derive(Debug, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub struct NewOrder {
    pub id: Uuid,
    pub side: Side,
    pub product: CurrencyPair,
    pub instruction: NewOrderInstruction,
}

#[derive(Debug, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub enum NewOrderInstruction {
    Limit {
        price: d128,
        quantity: d128,
        time_in_force: TimeInForce,
    }
}

// Market buy orders are placed in one of two ways for each exchange, 
// and so they're tricky (impossible?) to implement properly in an
// abstracted way. This doesn't even matter at the moment because 
// we should only need Limit orders for arbitrage.
//
// Market order examples:
// Gemini:
//   Sell 1BTC for CURRENT_PRICE
//   Buy  1000USD worth of BTC for CURRENT_PRICE
// Binance: 
//   Sell 1BTC for CURRENT_PRICE
//   Buy  1BTC for CURRENT_PRICE
// GDAX:
//   Sell 1BTC for CURRENT_PRICE
//   Buy  1000USD worth of BTC for CURRENT_PRICE
// Bfinex:
//   Sell 1BTC for CURRENT_PRICE
//   Buy  1BTC for CURRENT_PRICE
//
// pub struct NewMarketOrder {
//     pub id: String,
//     pub side: Side,
//     pub product: CurrencyPair,

//     pub funds: d128,
// }


#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum TimeInForce {
    /// GTT
    GoodTillTime(DateTime<Utc>),

    /// GFD
    GoodForDay,
    GoodForHour,
    GoodForMin,

    /// [GTC](https://en.wikipedia.org/wiki/Good_%27til_cancelled)
    GoodTillCancelled,

    /// [IOC](https://en.wikipedia.org/wiki/Immediate_or_cancel)
    ///
    /// Order must be immediately executed or cancelled. 
    /// Unlike `FillOrKill`, IOC orders can be partially filled.
    ImmediateOrCancel,

    /// [FOK](https://en.wikipedia.org/wiki/Fill_or_kill)
    /// 
    /// Order must be immediately executed or cancelled.
    /// Unlike `ImmediateOrCancel`, FOK orders *require* the full quantity to be executed.
    FillOrKill,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub enum OrderStatus {
    /// The order is active and can be filled; it also may already be partially
    /// filled.
    Open,

    /// The order has been completely filled and closed.
    Filled,

    /// The order was never `Open`; it was rejected for some specified reason.
    Rejected(String),

    /// The order was placed but has not yet been opened or rejected.
    Pending,

    /// The order was previously `Open` and voluntarily or involuntarily
    /// cancelled before being filled for some specified reason. 
    Closed(String),
}

#[derive(Debug, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Option<Uuid>,
    pub server_id: Option<String>,
    pub side: Side,
    pub product: CurrencyPair,
    pub status: OrderStatus,
    pub instruction: OrderInstruction,
}

#[derive(Debug, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub enum OrderInstruction {
    Limit {
        price: d128,
        original_quantity: d128,
        remaining_quantity: d128,
        time_in_force: TimeInForce,
    }
}

impl From<NewOrder> for Order {
    fn from(new_order: NewOrder) -> Self {
        Order {
            id: Some(new_order.id),
            server_id: None,
            side: new_order.side,
            product: new_order.product,
            status: OrderStatus::Pending,
            instruction: new_order.instruction.into(),
        }
    }
}

impl From<NewOrderInstruction> for OrderInstruction {
    fn from(new_order_instruction: NewOrderInstruction) -> Self {
        match new_order_instruction {
            NewOrderInstruction::Limit{price, quantity, time_in_force} => {
                OrderInstruction::Limit {
                    price: price,
                    original_quantity: quantity,
                    remaining_quantity: quantity,
                    time_in_force: time_in_force,
                }
            }
        }
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum Side {
    Ask,
    Bid,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub struct Offer {
    pub price: d128,
    pub supply: d128,
}

impl Offer {
    pub fn new(price: d128, supply: d128) -> Self {
        Offer { price, supply }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Orderbook {
    pub bids: Vec<Offer>,
    pub asks: Vec<Offer>,
}

impl Orderbook {
    pub fn remove(&mut self, side: Side, offer: Offer) {
        let current_offer = match side {
            Side::Ask => self.asks
                .binary_search_by_key(&offer.price, |offer| offer.price),
            Side::Bid => self.bids
                .binary_search_by_key(&offer.price, |offer| offer.price),
        };

        match (side, current_offer) {
            (Side::Ask, Ok(current_offer)) => {
                self.asks.remove(current_offer);
            }
            (Side::Bid, Ok(current_offer)) => {
                self.bids.remove(current_offer);
            }
            (_, Err(_)) => {
                panic!("Tried to remove an offer that doesn't exist");
            }
        }
    }

    pub fn add_or_update(&mut self, side: Side, offer: Offer) {
        let current_offer = match side {
            Side::Ask => self.asks
                .binary_search_by_key(&offer.price, |offer| offer.price),
            Side::Bid => self.bids
                .binary_search_by_key(&offer.price, |offer| offer.price),
        };

        match (side, current_offer) {
            (Side::Ask, Ok(current_offer)) => {
                self.asks[current_offer].supply = offer.supply;
            }
            (Side::Bid, Ok(current_offer)) => {
                self.bids[current_offer].supply = offer.supply;
            }
            (Side::Ask, Err(new_offer)) => {
                self.asks.insert(new_offer, offer);
            }
            (Side::Bid, Err(new_offer)) => {
                self.bids.insert(new_offer, offer);
            }
        }
    }

    pub fn highest_bid(&self) -> Option<Offer> {
        self.bids.last().cloned()
    }

    pub fn lowest_ask(&self) -> Option<Offer> {
        self.asks.first().cloned()
    }

    pub fn supply(&self) -> d128 {
        self.asks
            .iter()
            .fold(d128::zero(), |acc, offer| acc + offer.supply)
    }

    pub fn demand(&self) -> d128 {
        self.bids
            .iter()
            .fold(d128::zero(), |acc, offer| acc + offer.supply)
    }
}

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct Market {
    pub product: CurrencyPair,
    pub orderbook: Orderbook,

    /// Public trades; not specific to any user.
    pub trades: Vec<Trade>,
}

impl Market {
    pub fn new(product: &CurrencyPair) -> Self {
        Market {
            product: product.clone(),
            orderbook: Orderbook::default(),
            trades: Vec::with_capacity(256),
        }
    }
}

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct Exchange {
    pub id: ID,
    pub name: String,
    pub markets: Vec<Market>,
    pub orders: Vec<Order>,
}

impl Exchange {
    pub fn new(id: ID, name: String) -> Self {
        Exchange {
            id: id,
            name: name,
            markets: Vec::with_capacity(16),
            orders: Vec::with_capacity(32),
        }
    }

    pub fn add_market(&mut self, product: &CurrencyPair) {
        if self.markets.iter().any(|market| &market.product == product) {
            panic!("Market already exists");
        } else {
            self.markets.push(Market::new(product));
        }
    }

    pub fn market(&self, product: &CurrencyPair) -> Option<&Market> {
        self.markets
            .iter()
            .find(|market| &market.product == product)
    }

    pub fn market_mut(&mut self, product: &CurrencyPair) -> Option<&mut Market> {
        self.markets
            .iter_mut()
            .find(|market| &market.product == product)
    }

    pub fn apply(&mut self, event: ExchangeEvent) {
        match event {
            ExchangeEvent::Heartbeat => {},
            ExchangeEvent::OrderbookOfferUpdated(product, side, offer) => self.market_mut(&product)
                .unwrap()
                .orderbook
                .add_or_update(side, offer),
            ExchangeEvent::OrderbookOfferRemoved(product, side, offer) => self.market_mut(&product)
                .unwrap()
                .orderbook
                .remove(side, offer),
            ExchangeEvent::MarketAdded(product) => self.add_market(&product),
            ExchangeEvent::TradeExecuted(product, trade) => {
                self.market_mut(&product).unwrap().trades.push(trade)
            }
            ExchangeEvent::OrderAdded(order) => self.orders.push(order),
            ExchangeEvent::OrderOpened(order) => self.orders.push(order),
            ExchangeEvent::OrderFilled(order) => {
                match self.orders.iter().position(|o| o.id == order.id) {
                    Some(o) => self.orders[o] = order,
                    None => panic!(),
                }
            }
            ExchangeEvent::OrderClosed(order) => {
                match self.orders.iter().position(|o| o.id == order.id) {
                    Some(o) => {
                        self.orders.remove(o);
                    }
                    None => panic!(),
                }
            }
            ExchangeEvent::Batch(events) => {
                for event in events {
                    self.apply(event)
                }
            }
            ExchangeEvent::Unimplemented(event) => {}
        }
    }
}

// #[derive(Debug, Serialize, Clone, Default)]
// pub struct Economy {
//     pub exchanges: Vec<Exchange>,
// }

// impl Economy {
//     pub fn add_exchange(&mut self, id: ID, name: String) {
//         match self.exchanges.binary_search_by_key(&id, |exchange| exchange.id) {
//             Ok(exchange) => panic!("Exchange already exists"),
//             Err(new_exchange) => {self.exchanges.insert(new_exchange, Exchange::new(id, name));},
//         }
//     }

//     pub fn exchange(&self, id: ID) -> Option<&Exchange> {
//         match self.exchanges.binary_search_by_key(&id, |exchange| exchange.id) {
//             Ok(exchange) => self.exchanges.get(exchange),
//             Err(_) => None,
//         }
//     }

//     pub fn exchange_mut(&mut self, id: ID) -> Option<&mut Exchange> {
//         match self.exchanges.binary_search_by_key(&id, |exchange| exchange.id) {
//             Ok(exchange) => self.exchanges.get_mut(exchange),
//             Err(_) => None,
//         }
//     }

//     pub fn apply(&mut self, event: EconomyEvent) {
//         match event {
//             EconomyEvent::ExchangeAdded(id, name) => self.add_exchange(id, name),
//             EconomyEvent::ExchangeUpdated(id, event) => self.exchange_mut(id).unwrap().apply(event),
//         }
//     }
// }

// pub enum EconomyEvent {
//     ExchangeAdded(ID, String),
//     ExchangeUpdated(ID, MarketEvent),
// }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ExchangeEvent {
    Heartbeat,
    MarketAdded(CurrencyPair),
    OrderbookOfferUpdated(CurrencyPair, Side, Offer),
    OrderbookOfferRemoved(CurrencyPair, Side, Offer),
    TradeExecuted(CurrencyPair, Trade),
    OrderAdded(Order),
    OrderOpened(Order),
    OrderFilled(Order),
    OrderClosed(Order),
    Unimplemented(String),
    Batch(Vec<ExchangeEvent>)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ExchangeCommand {
    PlaceOrder(NewOrder),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ExchangeMessage {
    Event(ExchangeEvent),
    Command(ExchangeCommand),
}