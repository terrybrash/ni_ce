use chrono::{DateTime, Utc};
use num_traits::*;
use rust_decimal::Decimal as d128;
use std::fmt;
use std::iter::FromIterator;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use uuid::Uuid;

pub type ID = i64;

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Credential {
    pub secret: String,
    pub key: String,
    pub password: Option<String>,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
pub struct CurrencyPair(pub Currency, pub Currency);

impl CurrencyPair {
    pub fn base(&self) -> Currency {
        let &CurrencyPair(base, _) = self;
        base
    }

    pub fn quote(&self) -> Currency {
        let &CurrencyPair(_, quote) = self;
        quote
    }
}

pub const BTCUSD: CurrencyPair = CurrencyPair(Currency::BTC, Currency::USD);
pub const ETHBTC: CurrencyPair = CurrencyPair(Currency::ETH, Currency::BTC);
pub const BTCUSDT: CurrencyPair = CurrencyPair(Currency::BTC, Currency::USDT);

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
pub enum Currency {
    ADX,
    AE,
    AION,
    ANS,
    ANT,
    AST,
    BAT,
    BCAP,
    BCH,
    BMC,
    BNT,
    BTC,
    CFI,
    CVC,
    DASH,
    DCT,
    DGD,
    DNT,
    DOGE,
    EDG,
    ENG,
    EOS,
    ETC,
    ETH,
    EUR,
    GBG,
    GBP,
    GNO,
    GNT,
    GOLOS,
    GUP,
    HMQ,
    ICN,
    INCNT,
    IND,
    INS,
    KICK,
    KNC,
    LTC,
    LUN,
    MANA,
    MCO,
    MGO,
    MLN,
    MYST,
    NET,
    NEU,
    OAX,
    OMG,
    PAY,
    PLN,
    PLU,
    PRO,
    PTOY,
    QRL,
    QTUM,
    REP,
    REQ,
    RLC,
    ROUND,
    RUB,
    SALT,
    SAN,
    SBD,
    SNGLS,
    SNM,
    SNT,
    SRN,
    STEEM,
    STORJ,
    STX,
    TAAS,
    TIME,
    TKN,
    TNT,
    TRST,
    TRX,
    UAHPAY,
    USD,
    USDT,
    VEN,
    VSL,
    WAVES,
    WINGS,
    XID,
    XMR,
    XMRG,
    XRP,
    XXX,
    XZC,
    ZEC,
    ZRX,
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct ParseCurrencyError(String);

impl FromStr for Currency {
    type Err = ParseCurrencyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const CURRENCIES: [(&'static str, Currency); 92] = [
            ("ADX", Currency::ADX),
            ("AE", Currency::AE),
            ("AION", Currency::AION),
            ("ANS", Currency::ANS),
            ("ANT", Currency::ANT),
            ("AST", Currency::AST),
            ("BAT", Currency::BAT),
            ("BCAP", Currency::BCAP),
            ("BCH", Currency::BCH),
            ("BMC", Currency::BMC),
            ("BNT", Currency::BNT),
            ("BTC", Currency::BTC),
            ("CFI", Currency::CFI),
            ("CVC", Currency::CVC),
            ("DASH", Currency::DASH),
            ("DCT", Currency::DCT),
            ("DGD", Currency::DGD),
            ("DNT", Currency::DNT),
            ("DOGE", Currency::DOGE),
            ("EDG", Currency::EDG),
            ("ENG", Currency::ENG),
            ("EOS", Currency::EOS),
            ("ETC", Currency::ETC),
            ("ETH", Currency::ETH),
            ("EUR", Currency::EUR),
            ("GBG", Currency::GBG),
            ("GBP", Currency::GBP),
            ("GNO", Currency::GNO),
            ("GNT", Currency::GNT),
            ("GOLOS", Currency::GOLOS),
            ("GUP", Currency::GUP),
            ("HMQ", Currency::HMQ),
            ("ICN", Currency::ICN),
            ("INCNT", Currency::INCNT),
            ("IND", Currency::IND),
            ("INS", Currency::INS),
            ("KICK", Currency::KICK),
            ("KNC", Currency::KNC),
            ("LTC", Currency::LTC),
            ("LUN", Currency::LUN),
            ("MANA", Currency::MANA),
            ("MCO", Currency::MCO),
            ("MGO", Currency::MGO),
            ("MLN", Currency::MLN),
            ("MYST", Currency::MYST),
            ("NET", Currency::NET),
            ("NEU", Currency::NEU),
            ("OAX", Currency::OAX),
            ("OMG", Currency::OMG),
            ("PAY", Currency::PAY),
            ("PLN", Currency::PLN),
            ("PLU", Currency::PLU),
            ("PRO", Currency::PRO),
            ("PTOY", Currency::PTOY),
            ("QRL", Currency::QRL),
            ("QTUM", Currency::QTUM),
            ("REP", Currency::REP),
            ("REQ", Currency::REQ),
            ("RLC", Currency::RLC),
            ("ROUND", Currency::ROUND),
            ("RUB", Currency::RUB),
            ("SALT", Currency::SALT),
            ("SAN", Currency::SAN),
            ("SBD", Currency::SBD),
            ("SNGLS", Currency::SNGLS),
            ("SNM", Currency::SNM),
            ("SNT", Currency::SNT),
            ("SRN", Currency::SRN),
            ("STEEM", Currency::STEEM),
            ("STORJ", Currency::STORJ),
            ("STX", Currency::STX),
            ("TAAS", Currency::TAAS),
            ("TIME", Currency::TIME),
            ("TKN", Currency::TKN),
            ("TNT", Currency::TNT),
            ("TRST", Currency::TRST),
            ("TRX", Currency::TRX),
            ("UAHPAY", Currency::UAHPAY),
            ("USD", Currency::USD),
            ("USDT", Currency::USDT),
            ("VEN", Currency::VEN),
            ("VSL", Currency::VSL),
            ("WAVES", Currency::WAVES),
            ("WINGS", Currency::WINGS),
            ("XID", Currency::XID),
            ("XMR", Currency::XMR),
            ("XMRG", Currency::XMRG),
            ("XRP", Currency::XRP),
            ("XXX", Currency::XXX),
            ("XZC", Currency::XZC),
            ("ZEC", Currency::ZEC),
            ("ZRX", Currency::ZRX),
        ];

        for &(string, currency) in CURRENCIES.iter() {
            if string.eq_ignore_ascii_case(s) {
                return Ok(currency);
            }
        }
        Err(ParseCurrencyError(format!("couldn't parse \"{}\"", s)))
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

impl Balance {
    pub fn new(currency: Currency, balance: d128) -> Self {
        Balance { currency, balance }
    }
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
    },
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
    },
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
            NewOrderInstruction::Limit {
                price,
                quantity,
                time_in_force,
            } => OrderInstruction::Limit {
                price: price,
                original_quantity: quantity,
                remaining_quantity: quantity,
                time_in_force: time_in_force,
            },
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

#[derive(Debug, Hash, PartialEq, Eq, Ord, PartialOrd, Clone, Serialize, Deserialize)]
pub struct Offer {
    pub price: d128,
    pub quantity: d128,
}

impl Offer {
    pub fn new(price: d128, quantity: d128) -> Self {
        Offer { price, quantity }
    }

    pub fn total(&self) -> d128 {
        self.price * self.quantity
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Asks(Vec<Offer>);

impl Asks {
    pub fn with_capacity(capacity: usize) -> Self {
        Asks(Vec::with_capacity(capacity))
    }

    pub fn add_or_update(&mut self, offer: Offer) {
        match self.binary_search_by_key(&offer.price, |offer| offer.price) {
            Ok(current_offer) => {
                self[current_offer].quantity = offer.quantity;
            }
            Err(new_offer) => {
                self.insert(new_offer, offer);
            }
        }
    }

    pub fn remove_by_price(&mut self, price: &d128) -> Option<Offer> {
        self.binary_search_by_key(price, |offer| offer.price)
            .ok()
            .map(|offer| self.remove(offer))
    }
}

impl Deref for Asks {
    type Target = Vec<Offer>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Asks {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromIterator<Offer> for Asks {
    fn from_iter<I>(offers: I) -> Self
    where
        I: IntoIterator<Item = Offer>,
    {
        let mut offers = offers.into_iter();

        let (size, _) = offers.size_hint();
        let mut asks = Asks::with_capacity(size);

        for offer in offers {
            asks.add_or_update(offer);
        }

        asks
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Bids(Vec<Offer>);

impl Bids {
    pub fn with_capacity(capacity: usize) -> Self {
        Bids(Vec::with_capacity(capacity))
    }

    pub fn add_or_update(&mut self, offer: Offer) {
        match self.binary_search_by_key(&offer.price, |offer| offer.price) {
            Ok(current_offer) => {
                self[current_offer].quantity = offer.quantity;
            }
            Err(new_offer) => {
                self.insert(new_offer, offer);
            }
        }
    }

    pub fn remove_by_price(&mut self, price: &d128) -> Option<Offer> {
        self.binary_search_by_key(price, |offer| offer.price)
            .ok()
            .map(|offer| self.remove(offer))
    }
}

impl Deref for Bids {
    type Target = Vec<Offer>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Bids {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromIterator<Offer> for Bids {
    fn from_iter<I>(offers: I) -> Self
    where
        I: IntoIterator<Item = Offer>,
    {
        let mut offers = offers.into_iter();

        let (size, _) = offers.size_hint();
        let mut bids = Bids::with_capacity(size);

        for offer in offers {
            bids.add_or_update(offer);
        }

        bids
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Orderbook {
    pub asks: Asks,
    pub bids: Bids,
}

impl Orderbook {
    pub fn new(asks: Asks, bids: Bids) -> Self {
        Orderbook { asks, bids }
    }

    pub fn remove(&mut self, side: Side, offer: &Offer) -> Option<Offer> {
        match side {
            Side::Ask => self.asks.remove_by_price(&offer.price),
            Side::Bid => self.bids.remove_by_price(&offer.price),
        }
    }

    pub fn add_or_update(&mut self, side: Side, offer: Offer) {
        match side {
            Side::Ask => self.asks.add_or_update(offer),
            Side::Bid => self.bids.add_or_update(offer),
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
            .fold(d128::zero(), |acc, offer| acc + offer.quantity)
    }

    pub fn demand(&self) -> d128 {
        self.bids
            .iter()
            .fold(d128::zero(), |acc, offer| acc + offer.quantity)
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
            ExchangeEvent::Heartbeat => {}
            ExchangeEvent::OrderbookOfferUpdated(product, side, offer) => {
                self.market_mut(&product)
                    .unwrap()
                    .orderbook
                    .add_or_update(side, offer);
            }
            ExchangeEvent::OrderbookOfferRemoved(product, side, offer) => {
                self.market_mut(&product)
                    .unwrap()
                    .orderbook
                    .remove(side, &offer);
            }
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
            ExchangeEvent::Batch(events) => for event in events {
                self.apply(event)
            },
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
    Batch(Vec<ExchangeEvent>),
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
