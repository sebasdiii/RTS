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

use amiquip::{Connection, ConsumerMessage, ConsumerOptions, QueueDeclareOptions};
use std::sync::{Arc, Mutex};
use std::thread;
use ui::{StockApp, StockUpdate};

fn main() {
    // Shared state for stock updates
    let updates = Arc::new(Mutex::new(Vec::new()));

    // RabbitMQ connection setup
    let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")
        .expect("Failed to connect to RabbitMQ");
    let channel = connection.open_channel(None).expect("Failed to open channel");

    // Start a thread for consuming stock updates
    let updates_clone = Arc::clone(&updates);
    thread::spawn(move || {
        consume_stock_updates(&channel, updates_clone);
    });

    // Launch the UI in the main thread
    let app = StockApp::new(updates);
    let options = eframe::NativeOptions::default();
    if let Err(e) = eframe::run_native("Stock Updates", options, Box::new(|_cc| Ok(Box::new(app)))) {
        eprintln!("Failed to run native: {}", e);
    }
}

// Function to consume stock updates
fn consume_stock_updates(channel: &amiquip::Channel, updates: Arc<Mutex<Vec<StockUpdate>>>) {
    let queue = channel
        .queue_declare("stock_updates", QueueDeclareOptions::default())
        .expect("Failed to declare queue");

    let consumer = queue
        .consume(ConsumerOptions::default())
        .expect("Failed to start consumer");

    println!("\n[Trader Monitoring Stock Updates...]\n");

    for message in consumer.receiver().iter() {
        match message {
            ConsumerMessage::Delivery(delivery) => {
                let stock_update = String::from_utf8_lossy(&delivery.body);
                println!("[Stock Update Received] {}", stock_update);

                // Parse the JSON string into a StockUpdate struct
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&stock_update) {
                    if let (Some(stock), Some(price), Some(availability)) = (
                        parsed["stock"].as_str(),
                        parsed["price"].as_f64(),
                        parsed["availability"].as_u64(),
                    ) {
                        let update = StockUpdate {
                            stock: stock.to_string(),
                            price,
                            availability: availability as u32,
                        };

                        // Store the update
                        let mut updates = updates.lock().unwrap();
                        if updates.len() > 5 {
                            updates.remove(0); // Keep only the last 10 updates
                        }
                        updates.push(update);
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
