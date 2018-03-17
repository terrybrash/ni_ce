use ccex;
use chrono;
use decimal::d128;
use std::thread;
use ExchangeBuilder;
use url::Url;
use reqwest;
use Exchange;
use ccex::api::{WebsocketClient, NeedsAuthentication, HttpClient};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use ccex::gdax::{Credential};
use ccex::{ExchangeEvent, ExchangeMessage, ExchangeCommand};

#[derive(Debug)]
pub struct Gdax {
    credential: ccex::gdax::Credential,
    rest_client: reqwest::Client,
    exchange: Arc<Mutex<ccex::Exchange>>,
    sender: mpsc::Sender<ExchangeMessage>,
}

impl Gdax {
    pub fn from_builder(builder: ExchangeBuilder) -> Self { 
        let mut rest_client = reqwest::Client::new();
        let credential = Credential {
            key: builder.credential.key.clone(),
            secret: builder.credential.secret.clone(),
            password: builder.credential.password.unwrap().clone(),
        };

        let (sender, receiver) = mpsc::channel();
        let exchange = Arc::new(Mutex::new(ccex::Exchange::new(0, "gdax".to_owned())));

        // Register the products
        let markets = builder.products.clone().into_iter().map(ExchangeEvent::MarketAdded).collect();
        sender.send(ExchangeMessage::Event(ExchangeEvent::Batch(markets)));

        // Get currently opened orders.
        let orders = rest::orders(&mut rest_client, &credential).into_iter().map(ccex::ExchangeEvent::OrderAdded).collect();
        sender.send(ExchangeMessage::Event(ExchangeEvent::Batch(orders)));

        {
            // open subscribe websocket and start thread
            let credential = credential.clone();
            let products = builder.products.clone();
            let sender = sender.clone();
            thread::spawn(move || ws::market_loop(sender, credential, products));
        }

        {
            // open an event loop that keeps the exchange updated
            let exchange = exchange.clone();
            let mut rest_client = reqwest::Client::new();
            let credential = credential.clone();
            thread::spawn(move || {
                for message in receiver.iter() {
                    match message {
                        ExchangeMessage::Event(event) => {
                            println!("{:?}", event);
                            let mut exchange = exchange.lock().unwrap();
                            exchange.apply(event);
                        }
                        ExchangeMessage::Command(ExchangeCommand::PlaceOrder(new_order)) => {
                            let request = ccex::gdax::rest::PlaceOrder::from(new_order)
                                .authenticate(&credential);

                            rest_client.send(Url::parse("https://api-public.sandbox.gdax.com").unwrap(), request).unwrap();
                        }
                    }
                }
            });
        }

        Gdax {
            credential,
            rest_client,
            exchange,
            sender,
        }
    }
}

impl Exchange for Gdax {
    fn name(&self) -> &'static str {
        "gdax"
    }

    // thottie: returning the order is kind of interesting. maybe have
    // Exchange just be used for making http requests and returning the
    // responses, with the responsibility on the caller whether to update the
    // Exchange object with the response
    fn place_order(&mut self, new_order: ccex::NewOrder) -> ccex::Order {
        self.sender.send(ExchangeMessage::Command(ExchangeCommand::PlaceOrder(new_order.clone())));
        ccex::Order::from(new_order)
        // let request = ccex::gdax::rest::PlaceOrder::from(new_order.clone())
        //     .authenticate(&self.credential);

        // self.rest_client.send(Url::parse("https://api-public.sandbox.gdax.com").unwrap(), request).unwrap();
        // new_order.into()
    }

    fn balances(&mut self) -> Vec<ccex::Balance> {
        let request = ccex::gdax::rest::GetAccounts::default()
            .authenticate(&self.credential);

        let accounts = self.rest_client.send(Url::parse("https://api-public.sandbox.gdax.com").unwrap(), request).unwrap();

        accounts.iter().map(|account| {
            ccex::Balance {
                currency: account.currency.into(),
                balance: account.balance.into(),
            }
        }).collect()
    }

    fn orders(&mut self) -> Vec<ccex::Order> {
        unimplemented!()
        // self.exchange.orders.clone()
    }

    fn exchange(&mut self) -> MutexGuard<ccex::Exchange> {
        self.exchange.lock().unwrap()
    }


}

mod rest {
    use ccex;
    use ccex::api::{HttpClient, NeedsAuthentication};
    use ccex::gdax::rest::{GetOrders};
    use ccex::gdax::{Credential};
    use url::Url;
    use std::convert::TryInto;

    pub fn orders<Client>(client: &mut Client, credential: &Credential) -> Vec<ccex::Order>
    where Client: HttpClient {
        let request = GetOrders::default().authenticate(&credential);
        client.send(Url::parse("https://api-public.sandbox.gdax.com").unwrap(), request).unwrap()
            .into_iter().filter_map(|order| order.try_into().ok()).collect()
    }
}

mod ws {
    use url::Url;

    use ccex;
    use ccex::gdax::ws::{Channel, Message, Subscribe, ChannelName};
    use ccex::gdax::{CurrencyPair, Credential};
    use ccex::{Side, ExchangeEvent, Offer, ExchangeMessage, ExchangeCommand};
    use ccex::api::{TungsteniteClient, WebsocketClient};
    use std::sync::mpsc::{Sender};

    pub fn market_loop(mut sender: Sender<ExchangeMessage>, credential: Credential, products: Vec<ccex::CurrencyPair>) {
        let products: Vec<CurrencyPair> = products.iter().map(|p| p.clone().into()).collect();
        let request = Subscribe::new(
            &products,
            &[Channel {
                name: ChannelName::User,
                products: products.clone(),
            }, Channel {
                name: ChannelName::Heartbeat,
                products: products.clone(),
            }, Channel {
                name: ChannelName::Level2,
                products: products.clone(),
            }],
            &credential);
        let mut client = TungsteniteClient::connect(Url::parse("wss://ws-feed-public.sandbox.gdax.com").unwrap(), request.clone()).unwrap();
        client.send(Message::Subscribe(request)).unwrap();

        // thottie: this is kind of nice. we're doing all of the non-trivial
        // conversions here where there's no 1:1 conversion that can be
        // implemented by From
        loop {
            match client.recv() {
                Ok(Message::Error(error)) => {
                    panic!("{:?}", error);
                }
                Ok(Message::Heartbeat(heartbeat)) => {
                    sender.send(ExchangeMessage::Event(ExchangeEvent::Heartbeat));
                }
                Ok(Message::L2Update(update)) => {
                    let product = update.product.into();
                    let events = update.changes.into_iter().map(|(side, price, quantity)| {
                        if quantity.is_zero() {
                            ExchangeEvent::OrderbookOfferRemoved(product, side.into(), Offer::new(price, quantity))
                        } else {
                            ExchangeEvent::OrderbookOfferUpdated(product, side.into(), Offer::new(price, quantity))
                        }
                    }).collect();
                    sender.send(ExchangeMessage::Event(ExchangeEvent::Batch(events)));
                }
                Ok(Message::Snapshot(snapshot)) => {
                    let product = snapshot.product.into();

                    let bids = snapshot.bids.into_iter().map(|(price, quantity)| {
                        ExchangeEvent::OrderbookOfferUpdated(product, Side::Bid, Offer::new(price, quantity))
                    });

                    let asks = snapshot.asks.into_iter().map(|(price, quantity)| {
                        ExchangeEvent::OrderbookOfferUpdated(product, Side::Ask, Offer::new(price, quantity))
                    });

                    let events = bids.chain(asks).collect();
                    sender.send(ExchangeMessage::Event(ExchangeEvent::Batch(events)));
                }
                // Ok(Message::Received(order)) => {
                //     match order.order_type {
                //         Some(OrderType::Limit) => ccex::OrderInstruction::Limit {
                //             price: order.price,
                //             original_quantity: order.size.unwrap(),
                //             remaining_quantity: 
                //         }
                //     }
                //     instruction: ccex::OrderInstruction {
                //         price: order.price,
                //         original_quantity: 
                //     }
                //     let order = ccex::Order {
                //         side: order.side.into(),
                //         product: product_id.into(),
                //     }
                // },
                // Ok(Message::Open(order)) => {

                // }
                Ok(message) => {
                    println!("UNHANDLED: {:?}", message);
                }
                Err(e) => {
                    panic!("market thread crashed: {:?}", e);
                }
            }
        }
    }
}