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
mod stock_data; // Import the stock_data module

use amiquip::{
    Connection, ConsumerMessage, ConsumerOptions, Exchange, Publish, QueueDeclareOptions,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use stock_data::initialize_stocks; // Import the initialize_stocks function
use ui::{StockApp, StockUpdate};

// Struct for Order
#[derive(Serialize, Deserialize, Debug)]
struct Order {
    order_id: u32,
    stock: String,
    action: String, // "Buy" or "Sell"
    quantity: u32,
    price: f64,
}

fn main() {
    // Shared state for unique order IDs
    let order_id = Arc::new(Mutex::new(0));

    // Shared map for latest stock prices
    let stock_prices = Arc::new(Mutex::new(HashMap::new()));

    // Spawn a thread to consume stock updates
    let stock_prices_clone = Arc::clone(&stock_prices);
    thread::spawn(move || {
        let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")
            .expect("Failed to connect to RabbitMQ");
        let channel = connection.open_channel(None).expect("Failed to open channel");

        consume_stock_updates(&channel, stock_prices_clone);
    });

    // Spawn a thread for generating and publishing orders
    let order_id_clone = Arc::clone(&order_id);
    let stock_prices_clone = Arc::clone(&stock_prices);
    thread::spawn(move || {
        let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")
            .expect("Failed to connect to RabbitMQ");
        let channel = connection.open_channel(None).expect("Failed to open channel");
        let exchange = Exchange::direct(&channel);

        // Initialize stock data
        let stock_list = initialize_stocks();

        loop {
            // Introduce randomness in order generation frequency
            let should_generate_order = rand::thread_rng().gen_bool(0.4); // 40% chance to generate an order
            if should_generate_order {
                let order = generate_order(
                    Arc::clone(&order_id_clone),
                    Arc::clone(&stock_prices_clone),
                    &stock_list,
                );
                let order_json = serde_json::to_string(&order).expect("Failed to serialize order");

                // Publish the order to RabbitMQ
                exchange
                    .publish(Publish::new(order_json.as_bytes(), "order_queue"))
                    .expect("Failed to publish order");

                println!("[Order Sent]: {:?}", order);
            }

            // Randomize delay before checking again
            let delay = rand::thread_rng().gen_range(5..10); // Random delay between 5 and 10 seconds
            thread::sleep(Duration::from_secs(delay));
        }
    });

    // Keep the main thread alive
    loop {
        thread::park();
    }
}

// Function to generate a random order using synchronized stock prices
fn generate_order(
    order_id: Arc<Mutex<u32>>,
    stock_prices: Arc<Mutex<HashMap<String, f64>>>,
    stock_list: &[stock_data::Stock], // Accept stock_list as a parameter
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

// Function to consume stock updates from RabbitMQ and update stock prices
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

                // Parse the stock update and store it in the map
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
