mod stock_data;
mod brokers;

use amiquip::{Connection, ConsumerMessage, ConsumerOptions, Exchange, Publish, QueueDeclareOptions,};
use brokers::{Broker, Order};
use rand::Rng;
use serde_json;
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use stock_data::initialize_stocks;
use std::time::Instant; 

fn main() {
    let start_time = Instant::now();
    let shutdown_time = Duration::from_secs(60); // 1 minute
    
    // Setup shared state and initialize brokers
    let (order_id, stock_prices, receiver, brokers) = setup_shared_state_and_brokers();

    // Start threads for stock updates, order processing, and order generation
    start_stock_updates_thread(Arc::clone(&stock_prices));
    start_order_processing_thread(receiver);
    start_order_generation_thread(brokers, Arc::clone(&order_id), Arc::clone(&stock_prices));

    // // Keep the main thread alive
    // keep_main_thread_alive();
    // Market will close after 1 minute
    println!("Market Open!");

    while Instant::now() - start_time < shutdown_time {
        thread::sleep(Duration::from_secs(1)); // Check the timer every second
    }

    println!("Market Closed!");
}

// Function to set up shared state and initialize brokers
fn setup_shared_state_and_brokers(
    ) -> (
    Arc<Mutex<u32>>,
    Arc<Mutex<HashMap<String, f64>>>,
    mpsc::Receiver<Order>,
    Vec<Broker>,
    ) {
    // Shared state for unique order IDs
    let order_id = Arc::new(Mutex::new(0));

    // Shared map for latest stock prices
    let stock_prices = Arc::new(Mutex::new(
        initialize_stocks()
            .into_iter()
            .map(|stock| (stock.name.clone(), stock.price))
            .collect::<HashMap<String, f64>>(),
    ));

    // Communication channel between brokers and stock system
    let (sender, receiver) = mpsc::channel::<Order>();

    // Initialize brokers with the stock_prices argument
    let brokers: Vec<Broker> = (1..=3)
        .map(|id| Broker::new(id, sender.clone(), Arc::clone(&stock_prices)))
        .collect();

    (order_id, stock_prices, receiver, brokers)
}

// Function to start the thread that consumes stock updates
fn start_stock_updates_thread(stock_prices: Arc<Mutex<HashMap<String, f64>>>) {
    thread::spawn(move || {
        let mut connection =
            Connection::insecure_open("amqp://guest:guest@localhost:5672")
                .expect("Failed to connect to RabbitMQ");
        let channel = connection
            .open_channel(None)
            .expect("Failed to open channel");

        consume_stock_updates(&channel, stock_prices);
    });
}

// Function to start the thread that processes orders from brokers
fn start_order_processing_thread(receiver: mpsc::Receiver<Order>) {
    thread::spawn(move || {
        let mut connection =
            Connection::insecure_open("amqp://guest:guest@localhost:5672")
                .expect("Failed to connect to RabbitMQ");
        let channel = connection
            .open_channel(None)
            .expect("Failed to open channel");
        let exchange = Exchange::direct(&channel);

        for order in receiver {
            let order_json =
                serde_json::to_string(&order).expect("Failed to serialize order");

            exchange
                .publish(Publish::new(order_json.as_bytes(), "order_queue"))
                .expect("Failed to publish order");

            println!("[Stock System] Order Sent: {:?}\n\n--------------------------------------------------------------------------\n", order);
        }
    });
}

fn start_order_generation_thread(
    mut brokers: Vec<Broker>,
    order_id: Arc<Mutex<u32>>,
    stock_prices: Arc<Mutex<HashMap<String, f64>>>,
    ) {
    thread::spawn(move || {
        let stock_list = initialize_stocks();

        loop {
            let mut rng = rand::thread_rng();

            // Randomly select a broker
            let broker_id = rng.gen_range(0..brokers.len());
            let broker = &mut brokers[broker_id];

            // Generate a random order
            let order = generate_order(
                Arc::clone(&order_id),
                Arc::clone(&stock_prices),
                &stock_list,
            );

            println!("--------------------------------------------------------------------------\n[Client] Order Sent: {:?}\n", order);

            println!("[Broker] Order Received: {:?}\n--------------------------------------------------------------------------", order);

            // Process the order based on its type
            match order.order_type.as_str() {
                "Market" => broker.handle_order(order), // Market order
                "Limit" => process_limit_order(order, Arc::clone(&stock_prices), broker.sender.clone()), // Limit order
                _ => println!("[Client] Unknown order type: {:?}", order),
            }

            // Random delay between order generation
            let delay = rng.gen_range(5..10);
            thread::sleep(Duration::from_secs(delay));
        }
    });
}

// Function to generate a random order
fn generate_order(
    order_id: Arc<Mutex<u32>>,
    stock_prices: Arc<Mutex<HashMap<String, f64>>>,
    stock_list: &[stock_data::Stock],
    ) -> Order {
    let mut rng = rand::thread_rng();
    let stock = stock_list[rng.gen_range(0..stock_list.len())].name.clone();

    let action = if rng.gen_bool(0.5) {
        "Buy"
    } else {
        "Sell"
    }
    .to_string();
    let quantity = rng.gen_range(1..100); // Random quantity between 1 and 100

    // Decide randomly between Market and Limit order types
    let order_type = if rng.gen_bool(0.5) {
        "Market".to_string()
    } else {
        "Limit".to_string()
    };

    // Get the synchronized stock price
    let price = if order_type == "Limit" {
        // For limit orders, calculate a random limit price +/- 10% of the current price
        let base_price = {
            let prices = stock_prices.lock().unwrap();
            prices.get(&stock).copied().unwrap_or(0.0) // Default price if not found
        };
        let price_fluctuation = rng.gen_range(-10..=10) as f64 / 100.0; // Random fluctuation between -10% and +10%
        ((base_price * (1.0 + price_fluctuation)) * 100.0).round() / 100.0 // Ensure price is non-negative
    } else {
        let current_price = {
            let prices = stock_prices.lock().unwrap();
            prices.get(&stock).copied().unwrap_or(0.0)
        };
        (current_price * 100.0).round() / 100.0 // Ensure 2 decimal places
    };

    // Generate a unique order ID
    let mut id = order_id.lock().unwrap();
    *id += 1;

    Order {
        order_id: *id,
        stock,
        action,
        quantity,
        price,
        order_type,
    }
}

fn consume_stock_updates(
    channel: &amiquip::Channel,
    stock_prices: Arc<Mutex<HashMap<String, f64>>>,
    ) {
    let queue = channel
        .queue_declare("stock_updates", QueueDeclareOptions::default())
        .expect("Failed to declare queue");

    let consumer = queue
        .consume(ConsumerOptions::default())
        .expect("Failed to start consumer");

    println!("[Stock Update Monitor Started]");
    println!("--------------------------------------------------------------------------");

    for message in consumer.receiver().iter() {
        match message {
            ConsumerMessage::Delivery(delivery) => {
                let stock_update = String::from_utf8_lossy(&delivery.body);

                if let Ok(parsed) =
                    serde_json::from_str::<serde_json::Value>(&stock_update)
                {
                    if let (Some(stock), Some(price), Some(availability)) = (
                        parsed["Stock"].as_str(),
                        parsed["Price"].as_f64(),
                        parsed["Availability"].as_u64(),
                    ) {
                        // Update stock prices
                        {
                            let mut prices = stock_prices.lock().unwrap();
                            prices.insert(stock.to_string(), price);
                        }

                        // Print formatted stock update
                        println!(
                            "Stock: {:<10} | New Price: {:<8.2} | Availability: {}",
                            stock, price, availability
                        );
                    }
                }

                consumer
                    .ack(delivery)
                    .expect("Failed to acknowledge message");

            }
            other => {
                println!("Consumer ended: {:?}", other);
                break;
            }
        }
    }
}

fn process_limit_order(
    order: Order,
    stock_prices: Arc<Mutex<HashMap<String, f64>>>,
    sender: mpsc::Sender<Order>, // Add sender to send order to stock system
    ) {
    let stock_name = order.stock.clone();

    // Spawn a separate thread to monitor stock prices for the limit condition
    thread::spawn(move || {
        loop {
            let current_price = {
                let prices = stock_prices.lock().unwrap();
                *prices.get(&stock_name).unwrap_or(&0.0)
            };

            // Check if the limit condition is met
            let condition_met = match order.action.as_str() {
                "Buy" => current_price <= order.price, // Buy if price is <= limit
                "Sell" => current_price >= order.price, // Sell if price is >= limit
                _ => false,
            };

            if condition_met {
                println!(
                    "[Broker] Limit order condition met: Stock: {}, Action: {}, Price: {:.2}, Limit: {:.2}\n",
                    stock_name, order.action, current_price, order.price
                );

                // Send the order to the stock system
                sender.send(order.clone()).expect("Failed to send limit order to stock system");
                println!(
                    "[Broker] Limit order sent to stock system: Stock: {}, Action: {}, Quantity: {}, Price: {:.2}\n--------------------------------------------------------------------------\n",
                    stock_name, order.action, order.quantity, order.price
                );
                break;
            }

            // Sleep for a while before rechecking
            thread::sleep(Duration::from_secs(2));
        }
    });
}

// Function to keep the main thread alive
fn keep_main_thread_alive() {
    loop {
        thread::park();
    }
}

