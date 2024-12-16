mod stock_data;

use amiquip::{Connection, ConsumerMessage, ConsumerOptions, Exchange, Publish, QueueDeclareOptions};
use rand::Rng;
use stock_data::{initialize_stocks, Stock};
use std::sync::{Arc, Mutex};
use std::{thread, time::Duration};

fn main() {
    // Shared stock data
    let stocks = Arc::new(Mutex::new(initialize_stocks()));

    // Start the thread to periodically publish stock updates
    let stocks_clone = Arc::clone(&stocks);
    thread::spawn(move || {
        // RabbitMQ connection setup
        let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")
            .expect("Failed to connect to RabbitMQ");

        // Open a channel for publishing stock updates
        let channel = connection.open_channel(None).expect("Failed to open channel");
        let exchange = Exchange::direct(&channel);

        publish_stock_updates(&stocks_clone, &exchange);
    });

    // Start the thread to consume orders
    let stocks_clone = Arc::clone(&stocks);
    thread::spawn(move || {
        // RabbitMQ connection setup
        let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")
            .expect("Failed to connect to RabbitMQ");

        // Open a channel for consuming orders
        let channel = connection.open_channel(None).expect("Failed to open channel");

        consume_orders(&channel, stocks_clone);
    });

    // Start the thread to apply random events
    let stocks_clone = Arc::clone(&stocks);
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(20)); // Trigger random events every 20 seconds
            apply_random_event(&stocks_clone);
        }
    });

    // Keep the main thread alive for future extensions
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

// Function to publish stock updates every 5 seconds
fn publish_stock_updates(stocks: &Arc<Mutex<Vec<Stock>>>, exchange: &Exchange) {
    loop {
        {
            // Lock the stocks for reading
            let stocks = stocks.lock().unwrap();
            for stock in stocks.iter() {
                let message = format!(
                    "{{\"stock\": \"{}\", \"price\": {:.2}, \"availability\": {}}}",
                    stock.name, stock.price, stock.availability
                );

                // Publish the stock update to the queue
                exchange
                    .publish(Publish::new(message.as_bytes(), "stock_updates"))
                    .expect("Failed to publish stock update");

                println!("[Stock Sent] {}", message);
            }
        }

        // Simulate stock price fluctuations
        {
            let mut stocks = stocks.lock().unwrap();
            for stock in stocks.iter_mut() {
                stock.fluctuate_price();
            }
        }

        thread::sleep(Duration::from_secs(5)); // Update every 5 seconds
    }
}

// Function to consume orders from the queue
fn consume_orders(channel: &amiquip::Channel, stocks: Arc<Mutex<Vec<Stock>>>) {
    let queue = channel
        .queue_declare("order_queue", QueueDeclareOptions::default())
        .expect("Failed to declare queue");

    let consumer = queue
        .consume(ConsumerOptions::default())
        .expect("Failed to start consumer");

    println!("\n[Stock System Monitoring Orders...]\n");

    for message in consumer.receiver().iter() {
        match message {
            ConsumerMessage::Delivery(delivery) => {
                let order_data = String::from_utf8_lossy(&delivery.body);
                println!("[Order Received] {}", order_data);

                // Process the order
                if let Ok(order) = serde_json::from_str::<serde_json::Value>(&order_data) {
                    let stock_name = order["stock"].as_str().unwrap_or("");
                    let action = order["action"].as_str().unwrap_or("");
                    let quantity = order["quantity"].as_u64().unwrap_or(0);

                    // Update stock availability based on the order
                    let mut stocks = stocks.lock().unwrap();
                    if let Some(stock) = stocks.iter_mut().find(|s| s.name == stock_name) {
                        match action {
                            "Buy" => {
                                if stock.availability >= quantity as u32 {
                                    stock.availability -= quantity as u32;
                                    println!(
                                        "[Order Processed] Stock: {}, Action: {}, Quantity: {}, Remaining: {}",
                                        stock_name, action, quantity, stock.availability
                                    );
                                } else {
                                    println!(
                                        "[Order Rejected] Insufficient stock for {}: Requested {}, Available {}",
                                        stock_name, quantity, stock.availability
                                    );
                                }
                            }
                            "Sell" => {
                                stock.availability += quantity as u32;
                                println!(
                                    "[Order Processed] Stock: {}, Action: {}, Quantity: {}, New Availability: {}",
                                    stock_name, action, quantity, stock.availability
                                );
                            }
                            _ => println!("[Order Error] Unknown action: {}", action),
                        }
                    } else {
                        println!("[Order Error] Stock not found: {}", stock_name);
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

// Function to apply random events
fn apply_random_event(stocks: &Arc<Mutex<Vec<Stock>>>) {
    let mut rng = rand::thread_rng();

    // Define some random events with their impact percentage
    let events = vec![
        ("US Election", 0.2),         // 20% increase or decrease
        ("Tech Bubble Burst", -0.3), // 30% decrease
        ("Interest Rate Hike", -0.1), // 10% decrease
        ("Major Product Launch", 0.25), // 25% increase
        ("Economic Boom", 0.15),     // 15% increase
        ("Pandemic News", -0.2),     // 20% decrease
    ];

    // Randomly select an event
    let (event_name, impact) = events[rng.gen_range(0..events.len())];
    println!("[Random Event Triggered]: {} with impact {:.2}%", event_name, impact * 100.0);

    // Apply the impact to all stocks
    let mut stocks = stocks.lock().unwrap();
    for stock in stocks.iter_mut() {
        let fluctuation = stock.price * impact;
        stock.price = (stock.price + fluctuation).max(1.0); // Ensure price stays above 1.0
    }

    println!("[Stock Prices Updated]: {:?}", stocks);
}
