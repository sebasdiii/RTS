// use amiquip::{Connection, ConsumerMessage, ConsumerOptions, QueueDeclareOptions};

// // Entry point
// fn main() {
//     // Setup RabbitMQ connection
//     let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")
//         .expect("Failed to connect to RabbitMQ");

//     // Open a channel for consuming stock updates
//     let channel = connection.open_channel(None).expect("Failed to open channel");

//     // Call the function to consume stock updates
//     consume_stock_updates(&channel);
// }

// // Function to consume stock updates and display them
// fn consume_stock_updates(channel: &amiquip::Channel) {
//     // Declare the queue for stock updates
//     let queue = channel
//         .queue_declare("stock_updates", QueueDeclareOptions::default())
//         .expect("Failed to declare queue");

//     // Create a consumer to listen for stock updates
//     let consumer = queue
//         .consume(ConsumerOptions::default())
//         .expect("Failed to start consumer");

//     println!("\n[Trader Monitoring Stock Updates...]\n");

//     // Loop to receive and print stock updates
//     for message in consumer.receiver().iter() {
//         match message {
//             ConsumerMessage::Delivery(delivery) => {
//                 let stock_update = String::from_utf8_lossy(&delivery.body);
//                 println!("[Stock Update Received] {}", stock_update);

//                 // Acknowledge the message
//                 consumer.ack(delivery).expect("Failed to acknowledge message");
//             }
//             other => {
//                 println!("Consumer ended: {:?}", other);
//                 break;
//             }
//         }
//     }
// }


mod ui;
mod stock_data;
mod brokers; // Add the brokers module

use amiquip::{Connection, ConsumerMessage, ConsumerOptions, Exchange, Publish, QueueDeclareOptions};
use rand::Rng;
use serde_json;
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use brokers::{Broker, Order};
use stock_data::initialize_stocks;

fn main() {
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

    // Initialize brokers
    let mut brokers: Vec<Broker> = (1..=3)
        .map(|id| Broker::new(id, sender.clone()))
        .collect();

    // Thread to handle stock updates
    let stock_prices_clone = Arc::clone(&stock_prices);

    let order_id_clone = Arc::clone(&order_id);

    let stock_updates_ready = Arc::new(Mutex::new(false));

    thread::spawn(move || {
        let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")
            .expect("Failed to connect to RabbitMQ");
        let channel = connection.open_channel(None).expect("Failed to open channel");

        consume_stock_updates(&channel, stock_prices_clone);
    });

    // Thread to process orders from brokers to stock system
    thread::spawn(move || {
        let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")
            .expect("Failed to connect to RabbitMQ");
        let channel = connection.open_channel(None).expect("Failed to open channel");
        let exchange = Exchange::direct(&channel);

        for order in receiver {
            let order_json = serde_json::to_string(&order).expect("Failed to serialize order");

            exchange
                .publish(Publish::new(order_json.as_bytes(), "order_queue"))
                .expect("Failed to publish order");

            println!("[Stock System] Order Sent: {:?}", order);
        }
    });

    let stock_prices_clone = Arc::clone(&stock_prices);

    thread::spawn(move || {
        let stock_list = initialize_stocks();

        loop {
            // Randomly select a broker
            let mut rng = rand::thread_rng();
            let broker_id = rng.gen_range(0..3);
            let broker = &mut brokers[broker_id];

            // Generate a random order
            let order = generate_order(
                Arc::clone(&order_id_clone),
                Arc::clone(&stock_prices_clone),
                &stock_list,
            );

            println!("[Broker {}] Received order: {:?}", broker.id, order);
            broker.handle_order(order);

            // Random delay between order generation
            let delay = rng.gen_range(5..10);
            thread::sleep(Duration::from_secs(delay));
        }
    });

    // Keep the main thread alive
    loop {
        thread::park();
    }
}

// Function to generate a random order
fn generate_order(
        order_id: Arc<Mutex<u32>>,
        stock_prices: Arc<Mutex<HashMap<String, f64>>>,
        stock_list: &[stock_data::Stock],
    ) -> Order {
        let mut rng = rand::thread_rng();
        let stock = stock_list[rng.gen_range(0..stock_list.len())].name.clone();

        let action = if rng.gen_bool(0.5) { "Buy" } else { "Sell" }.to_string();
        let quantity = rng.gen_range(1..100); // Random quantity between 1 and 100

        // Get the synchronized stock price
        let price = {
            let prices = stock_prices.lock().unwrap();
            prices.get(&stock).copied().unwrap_or(0.0) // Default price if not found
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
        }
    }

fn consume_stock_updates(channel: &amiquip::Channel, stock_prices: Arc<Mutex<HashMap<String, f64>>>) {
    let queue = channel
        .queue_declare("stock_updates", QueueDeclareOptions::default())
        .expect("Failed to declare queue");

    let consumer = queue
        .consume(ConsumerOptions::default())
        .expect("Failed to start consumer");

    println!("[Trader Monitoring Stock Updates...]");

    for message in consumer.receiver().iter() {
        match message {
            ConsumerMessage::Delivery(delivery) => {
                let stock_update = String::from_utf8_lossy(&delivery.body);

                println!("[Stock Update Received]: {}", stock_update);

                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&stock_update) {
                    if let (Some(stock), Some(price)) = (
                        parsed["stock"].as_str(),
                        parsed["price"].as_f64(),
                    ) {
                        let mut prices = stock_prices.lock().unwrap();
                        prices.insert(stock.to_string(), price);
                        println!("[Stock Price Updated]: {} -> {:.2}", stock, price);
                    }
                }

                consumer.ack(delivery).expect("Failed to acknowledge message");
            }
            other => {
                println!("Consumer ended: {:?}", other);
                break;
            }
        }
    }
}

