// mod stock_data;

// use amiquip::{Connection, ConsumerMessage, ConsumerOptions, Exchange, Publish, QueueDeclareOptions};
// use rand::Rng;
// use stock_data::{initialize_stocks, Stock};
// use std::sync::{mpsc, Arc, Mutex};
// use std::thread;
// use std::time::{Duration,Instant};

// fn main() {
//     let start_time = Instant::now();
//     let shutdown_time = Duration::from_secs(60); // 1 minute

//     // Shared stock data
//     let shared_stock_data = Arc::new(Mutex::new(initialize_stocks()));
    
//     // Start the thread to periodically publish stock updates
//     let stock_data_for_publishing = Arc::clone(&shared_stock_data);
//     thread::spawn(move || {
//         // RabbitMQ connection setup
//         let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")
//             .expect("Failed to connect to RabbitMQ");

//         // Open a channel for publishing stock updates
//         let channel = connection.open_channel(None).expect("Failed to open channel");
//         let exchange = Exchange::direct(&channel);

//         publish_stock_updates(&stock_data_for_publishing, &exchange);
//     });

//     // Start the thread to consume orders
//     let stock_data_for_orders = Arc::clone(&shared_stock_data);
//     thread::spawn(move || {
//         // RabbitMQ connection setup
//         let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")
//             .expect("Failed to connect to RabbitMQ");

//         // Open a channel for consuming orders
//         let channel = connection.open_channel(None).expect("Failed to open channel");

//         consume_orders(&channel, stock_data_for_orders);
//     });

//     // Internal mpsc communication for stock updates
//     let (event_sender, event_receiver) = mpsc::channel::<(String, f64)>(); // Event: (Event Name, Impact %)

//     // Thread to listen for internal events and update stock data
//     let stock_data_for_receiver = Arc::clone(&shared_stock_data);
//     thread::spawn(move || {
//         process_stock_events(event_receiver, stock_data_for_receiver);
//     });

//     // Start the thread to apply random events
//     //let stock_data_for_events = Arc::clone(&shared_stock_data);
//     let sender_clone = event_sender.clone();
//     thread::spawn(move || {
//         loop {
//             thread::sleep(Duration::from_secs(20)); // Trigger random events every 20 seconds
//             apply_random_event(sender_clone.clone());
//         }
//     });
    
//     // // Keep the main thread alive for future extensions
//     // loop {
//     //     thread::sleep(Duration::from_secs(1));
//     // }

//     // Market will close after 1 minute
//     println!("Market Open!");

//     while Instant::now() - start_time < shutdown_time {
//         thread::sleep(Duration::from_secs(1)); // Check the timer every second
//     }

//     println!("Market Closed!");
// }




// // Function to publish stock updates every 5 seconds
// fn publish_stock_updates(stock_data: &Arc<Mutex<Vec<Stock>>>, exchange: &Exchange) {
//     loop {
//         {
//             // Lock the stocks for reading
//             let stock_data_locked = stock_data.lock().unwrap();
//             for stock in stock_data_locked.iter() {
//                 let message = format!(
//                     "{{\"Stock\": \"{}\", \"Price\": {:.2}, \"Availability\": {}}}",
//                     stock.name, stock.price, stock.availability
//                 );

//                 // Publish the stock update to the queue
//                 exchange
//                     .publish(Publish::new(message.as_bytes(), "stock_updates"))
//                     .expect("Failed to publish stock update");

//                 println!("[Stock Sent] {}", message);
//             }
//             println!("--------------------------------------------------------------------------")
//         }

//         // Simulate stock price fluctuations
//         {
//             let mut stock_data_locked = stock_data.lock().unwrap();
//             for stock in stock_data_locked.iter_mut() {
//                 stock.fluctuate_price();
//             }
//         }

//         thread::sleep(Duration::from_secs(5)); // Update every 5 seconds
//     }
// }





// // Function to consume orders from the queue
// fn consume_orders(channel: &amiquip::Channel, stock_data: Arc<Mutex<Vec<Stock>>>) {
//     let queue = channel
//         .queue_declare("order_queue", QueueDeclareOptions::default())
//         .expect("Failed to declare queue");

//     let consumer = queue
//         .consume(ConsumerOptions::default())
//         .expect("Failed to start consumer");

//     println!("\n[Stock System Monitoring Orders...]\n");

//     for message in consumer.receiver().iter() {
//         match message {
//             ConsumerMessage::Delivery(delivery) => {
//                 let order_data = String::from_utf8_lossy(&delivery.body);
//                 println!("[Order Received] {}", order_data);

//                 // Process the order
//                 if let Ok(order) = serde_json::from_str::<serde_json::Value>(&order_data) {
//                     let stock_name = order["stock"].as_str().unwrap_or("");
//                     let action = order["action"].as_str().unwrap_or("");
//                     let quantity = order["quantity"].as_u64().unwrap_or(0) as u32;

//                     // Update stock availability and price based on the order
//                     let mut stock_data_locked = stock_data.lock().unwrap();
//                     if let Some(stock) = stock_data_locked.iter_mut().find(|s| s.name == stock_name) {
//                         let percentage_of_availability = quantity as f64 / stock.availability as f64;

//                         match action {
//                             "Buy" => {
//                                 if stock.availability >= quantity {
//                                     stock.availability -= quantity;

//                                     // Price increases proportionally (capped at 15%)
//                                     let price_increase = stock.price
//                                         * (percentage_of_availability * 0.8).min(0.15); // Scale by 80%, max 15%
//                                     stock.price += price_increase;

//                                     println!(
//                                         "[Order Processed] Stock: {}, Action: {}, Quantity: {}, Remaining: {}, New Price: {:.2}\n",
//                                         stock_name, action, quantity, stock.availability, stock.price
//                                     );
//                                 } else {
//                                     println!(
//                                         "[Order Rejected] Insufficient stock for {}: Requested {}, Available {}",
//                                         stock_name, quantity, stock.availability
//                                     );
//                                 }
//                             }
//                             "Sell" => {
//                                 stock.availability += quantity;

//                                 // Price decreases proportionally (capped at 15%)
//                                 let price_decrease = stock.price
//                                     * (percentage_of_availability * 0.8).min(0.15); // Scale by 80%, max 15%
//                                 stock.price = (stock.price - price_decrease).max(1.0); // Ensure price stays positive

//                                 println!(
//                                     "[Order Processed] Stock: {}, Action: {}, Quantity: {}, New Availability: {}, New Price: {:.2}\n",
//                                     stock_name, action, quantity, stock.availability, stock.price
//                                 );
//                             }
//                             _ => println!("[Order Error] Unknown action: {}", action),
//                         }
//                     } else {
//                         println!("[Order Error] Stock not found: {}", stock_name);
//                     }
//                 }

//                 consumer.ack(delivery).expect("Failed to acknowledge message");
//             }
//             other => {
//                 println!("Consumer ended: {:?}", other);
//                 break;
//             }
//         }
//     }
// }

// // Function to apply random events (sends events via mpsc)
// fn apply_random_event(sender: mpsc::Sender<(String, f64)>) {
//     let mut rng = rand::thread_rng();

//     let events = vec![
//         ("US Election", 0.2),
//         ("Tech Bubble Burst", -0.3),
//         ("Interest Rate Hike", -0.1),
//         ("Major Product Launch", 0.25),
//         ("Economic Boom", 0.15),
//         ("Pandemic News", -0.2),
//     ];

//     let (event_name, impact) = events[rng.gen_range(0..events.len())];

//     println!(
//         "\n[Random Event Triggered]: {} with impact {:.2}%",
//         event_name,
//         impact * 100.0
//     );

//     // Send the event to the receiver thread
//     if let Err(e) = sender.send((event_name.to_string(), impact)) {
//         eprintln!("[Error] Failed to send event: {}", e);
//     }
// }

// // Receiver thread function to process events
// fn process_stock_events(receiver: mpsc::Receiver<(String, f64)>, stock_data: Arc<Mutex<Vec<Stock>>>) {
//     for (event_name, impact) in receiver {
//         println!("\n[Processing Event]: {} | Impact: {:.2}%", event_name, impact * 100.0);

//         let mut stock_data_locked = stock_data.lock().unwrap();
//         for stock in stock_data_locked.iter_mut() {
//             let fluctuation = stock.price * impact;
//             stock.price = (stock.price + fluctuation).max(1.0);
//         }

//         println!("--- Updated Stock Prices After Event ---");
//         for stock in stock_data_locked.iter() {
//             println!(
//                 "Stock: {:<10} | New Price: {:.2} | Availability: {}",
//                 stock.name, stock.price, stock.availability
//             );
//         }
//         println!("--------------------------------------------------------------------------");
//     }
// }

mod stock_data;

use amiquip::{Connection, ConsumerMessage, ConsumerOptions, Exchange, Publish, QueueDeclareOptions};
use rand::Rng;
use stock_data::{initialize_stocks, Stock};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let start_time = Instant::now();
    let shutdown_time = Duration::from_secs(60); // 1 minute

    // Initialize shared stock data and internal communication channel
    let shared_stock_data = Arc::new(Mutex::new(initialize_stocks()));
    let (event_sender, event_receiver) = mpsc::channel::<(String, f64)>(); // mpsc channel

    // Start system components
    start_stock_publisher(Arc::clone(&shared_stock_data));
    start_order_consumer(Arc::clone(&shared_stock_data));
    start_event_processor(event_receiver, Arc::clone(&shared_stock_data));
    start_random_event_trigger(event_sender);

    // Keep the main thread alive for 1 minute
    println!("Market Open!");
    run_market_timer(start_time, shutdown_time);
    println!("Market Closed!");
}

/// Start the stock publisher thread
fn start_stock_publisher(stock_data: Arc<Mutex<Vec<Stock>>>) {
    thread::spawn(move || {
        let mut connection =
            Connection::insecure_open("amqp://guest:guest@localhost:5672").expect("Failed to connect to RabbitMQ");
        let channel = connection.open_channel(None).expect("Failed to open channel");
        let exchange = Exchange::direct(&channel);

        publish_stock_updates(&stock_data, &exchange);
    });
}

/// Start the order consumer thread
fn start_order_consumer(stock_data: Arc<Mutex<Vec<Stock>>>) {
    thread::spawn(move || {
        let mut connection =
            Connection::insecure_open("amqp://guest:guest@localhost:5672").expect("Failed to connect to RabbitMQ");
        let channel = connection.open_channel(None).expect("Failed to open channel");

        consume_orders(&channel, stock_data);
    });
}

/// Start the internal event processor thread
fn start_event_processor(receiver: mpsc::Receiver<(String, f64)>, stock_data: Arc<Mutex<Vec<Stock>>>) {
    thread::spawn(move || {
        process_stock_events(receiver, stock_data);
    });
}

/// Start the random event trigger thread
fn start_random_event_trigger(sender: mpsc::Sender<(String, f64)>) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(20)); // Trigger random events every 20 seconds
        apply_random_event(sender.clone());
    });
}

/// Function to run the market timer
fn run_market_timer(start_time: Instant, shutdown_time: Duration) {
    while Instant::now() - start_time < shutdown_time {
        thread::sleep(Duration::from_secs(1));
    }
}

// Function to publish stock updates every 5 seconds
fn publish_stock_updates(stock_data: &Arc<Mutex<Vec<Stock>>>, exchange: &Exchange) {
    loop {
        {
            let stock_data_locked = stock_data.lock().unwrap();
            for stock in stock_data_locked.iter() {
                let message = format!(
                    "{{\"Stock\": \"{}\", \"Price\": {:.2}, \"Availability\": {}}}",
                    stock.name, stock.price, stock.availability
                );

                exchange
                    .publish(Publish::new(message.as_bytes(), "stock_updates"))
                    .expect("Failed to publish stock update");

                println!("[Stock Sent] {}", message);
            }
            println!("--------------------------------------------------------------------------");
        }

        {
            let mut stock_data_locked = stock_data.lock().unwrap();
            for stock in stock_data_locked.iter_mut() {
                stock.fluctuate_price();
            }
        }

        thread::sleep(Duration::from_secs(5));
    }
}

// Function to consume orders from the queue
fn consume_orders(channel: &amiquip::Channel, stock_data: Arc<Mutex<Vec<Stock>>>) {
    let queue = channel.queue_declare("order_queue", QueueDeclareOptions::default())
        .expect("Failed to declare queue");

    let consumer = queue.consume(ConsumerOptions::default())
        .expect("Failed to start consumer");

    println!("\n[Stock System Monitoring Orders...]\n");

    for message in consumer.receiver().iter() {
        match message {
            ConsumerMessage::Delivery(delivery) => {
                let order_data = String::from_utf8_lossy(&delivery.body);
                println!("[Order Received] {}", order_data);

                process_order(&order_data, &stock_data);
                consumer.ack(delivery).expect("Failed to acknowledge message");
            }
            other => {
                println!("Consumer ended: {:?}", other);
                break;
            }
        }
    }
}

// Function to process an individual order
fn process_order(order_data: &str, stock_data: &Arc<Mutex<Vec<Stock>>>) {
    if let Ok(order) = serde_json::from_str::<serde_json::Value>(order_data) {
        let stock_name = order["stock"].as_str().unwrap_or("");
        let action = order["action"].as_str().unwrap_or("");
        let quantity = order["quantity"].as_u64().unwrap_or(0) as u32;

        let mut stock_data_locked = stock_data.lock().unwrap();
        if let Some(stock) = stock_data_locked.iter_mut().find(|s| s.name == stock_name) {
            let percentage_of_availability = quantity as f64 / stock.availability as f64;

            match action {
                "Buy" => {
                    if stock.availability >= quantity {
                        stock.availability -= quantity;
                        stock.price += stock.price * (percentage_of_availability * 0.8).min(0.15);
                        println!("[Order Processed] Stock: {}, Action: {}, Quantity: {}, Remaining: {}, New Price: {:.2}\n", stock_name, action, quantity, stock.availability, stock.price);
                    } else {
                        println!("[Order Rejected] Insufficient stock for {}: Requested {}, Available {}", stock_name, quantity, stock.availability);
                    }
                }
                "Sell" => {
                    stock.availability += quantity;
                    stock.price = (stock.price - stock.price * (percentage_of_availability * 0.8).min(0.15)).max(1.0);
                    println!("[Order Processed] Stock: {}, Action: {}, Quantity: {}, New Availability: {}, New Price: {:.2}\n", stock_name, action, quantity, stock.availability, stock.price);
                }
                _ => println!("[Order Error] Unknown action: {}", action),
            }
        } else {
            println!("[Order Error] Stock not found: {}", stock_name);
        }
    }
}

// Function to apply random events
fn apply_random_event(sender: mpsc::Sender<(String, f64)>) {
    let mut rng = rand::thread_rng();
    let events = vec![
        ("US Election", 0.2),
        ("Tech Bubble Burst", -0.3),
        ("Interest Rate Hike", -0.1),
        ("Major Product Launch", 0.25),
        ("Economic Boom", 0.15),
        ("Pandemic News", -0.2),
    ];

    let (event_name, impact) = events[rng.gen_range(0..events.len())];
    println!("\n[Random Event Triggered]: {} with impact {:.2}%", event_name, impact * 100.0);

    if let Err(e) = sender.send((event_name.to_string(), impact)) {
        eprintln!("[Error] Failed to send event: {}", e);
    }
}

// Function to process events and update stock prices
fn process_stock_events(receiver: mpsc::Receiver<(String, f64)>, stock_data: Arc<Mutex<Vec<Stock>>>) {
    for (event_name, impact) in receiver {
        println!("\n[Processing Event]: {} | Impact: {:.2}%", event_name, impact * 100.0);

        let mut stock_data_locked = stock_data.lock().unwrap();
        for stock in stock_data_locked.iter_mut() {
            let fluctuation = stock.price * impact;
            stock.price = (stock.price + fluctuation).max(1.0);
        }

        println!("--- Updated Stock Prices After Event ---");
        for stock in stock_data_locked.iter() {
            println!("Stock: {:<10} | New Price: {:.2} | Availability: {}", stock.name, stock.price, stock.availability);
        }
        println!("--------------------------------------------------------------------------");
    }
}
