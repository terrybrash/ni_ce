use decimal::d128;
use std::fmt;

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
    pub supply: d128,
}

#[derive(Debug, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub maker_side: Side,
    pub price: d128,
    pub supply: d128,
}

#[derive(Debug, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub struct NewOrder {
    pub id: String,
    pub side: Side,
    pub product: CurrencyPair,
    pub price: d128,
    pub supply: d128,
}

#[derive(Debug, Hash, PartialEq, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub side: Side,
    pub product: CurrencyPair,
    pub price: d128,
    pub original_supply: d128,
    pub remaining_supply: d128,
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
            ExchangeEvent::OrderBooked(order) => self.orders.push(order),
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
    OrderBooked(Order),
    OrderFilled(Order),
    OrderClosed(Order),
    Unimplemented(String),
}
