mod stock_data;

use amiquip::{Connection, ConsumerMessage, ConsumerOptions, Exchange, Publish, QueueDeclareOptions};
use rand::Rng;
use stock_data::{initialize_stocks, Stock};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
/// Enum for stock updates
enum StockUpdate {
    RandomEvent { event_name: String, impact: f64 },
    PriceFluctuation { stock_name: String, fluctuation: f64 },
    Order { stock_name: String, action: String, quantity: u32 },
}
fn main() {
    let start_time = Instant::now();
    let shutdown_time = Duration::from_secs(60); // 1 minute

    // Shared stock data and mpsc channel
    let shared_stock_data = Arc::new(Mutex::new(initialize_stocks()));
    let (event_sender, event_receiver) = mpsc::channel::<StockUpdate>();

    // Start internal components
    start_stock_publisher(Arc::clone(&shared_stock_data));
    start_order_consumer(event_sender.clone());
    start_event_processor(event_receiver, Arc::clone(&shared_stock_data));
    start_random_event_trigger(event_sender.clone());
    start_price_fluctuator(event_sender);

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

        // Initial Publish (before the loop)
        {
            let stock_data_locked = stock_data.lock().unwrap();
            for stock in stock_data_locked.iter() {
                let message = format!(
                    "{{\"Stock\": \"{}\", \"Price\": {:.2}, \"Availability\": {}}}",
                    stock.name, stock.price, stock.availability
                );

                exchange
                    .publish(Publish::new(message.as_bytes(), "stock_updates"))
                    .expect("Failed to publish initial stock update");

                println!("[Stock Sent Initially] {}", message);
            }
        }

        loop {
            thread::sleep(Duration::from_secs(5)); // Publish updates every 5 seconds

            let stock_data_locked = stock_data.lock().unwrap();
            for stock in stock_data_locked.iter() {
                let message = format!(
                    "{{\"Stock\": \"{}\", \"Price\": {:.2}, \"Availability\": {}}}",
                    stock.name, stock.price, stock.availability
                );

                // Publish the stock update to RabbitMQ
                exchange
                    .publish(Publish::new(message.as_bytes(), "stock_updates"))
                    .expect("Failed to publish stock update");

                println!("[Stock Sent] {}", message);
            }
            //thread::sleep(Duration::from_secs(5)); // Publish updates every 5 seconds
            println!("--------------------------------------------------------------------------");
        }
    });
}

/// Start the order consumer thread
fn start_order_consumer(event_sender: mpsc::Sender<StockUpdate>) {
    thread::spawn(move || {
        let mut connection =
            Connection::insecure_open("amqp://guest:guest@localhost:5672").expect("Failed to connect to RabbitMQ");
        let channel = connection.open_channel(None).expect("Failed to open channel");

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

                    if let Ok(order) = serde_json::from_str::<serde_json::Value>(&order_data) {
                        let stock_name = order["stock"].as_str().unwrap_or("").to_string();
                        let action = order["action"].as_str().unwrap_or("").to_string();
                        let quantity = order["quantity"].as_u64().unwrap_or(0) as u32;

                        // Send order update via mpsc
                        event_sender.send(StockUpdate::Order {
                            stock_name,
                            action,
                            quantity,
                        }).expect("Failed to send order update");
                    }

                    consumer.ack(delivery).expect("Failed to acknowledge message");
                }
                other => {
                    println!("Consumer ended: {:?}", other);
                    break;
                }
            }
        }
    });
}

fn start_event_processor(receiver: mpsc::Receiver<StockUpdate>, stock_data: Arc<Mutex<Vec<Stock>>>) {
    thread::spawn(move || {
        for update in receiver {
            let mut stock_data_locked = stock_data.lock().unwrap();

            match update {
                // Process Random Events
                StockUpdate::RandomEvent { event_name, impact } => {
                    println!("\n[Processing Random Event]: {} | Impact: {:.2}%", event_name, impact * 100.0);
                    for stock in stock_data_locked.iter_mut() {
                        stock.price = (stock.price + stock.price * impact).max(1.0);
                    }
                }
                // Process Price Fluctuations
                StockUpdate::PriceFluctuation { stock_name, fluctuation } => {
                    if let Some(stock) = stock_data_locked.iter_mut().find(|s| s.name == stock_name) {
                        stock.price = (stock.price + stock.price * fluctuation).max(1.0);
                    }
                }
                // Process Orders (Buy/Sell)
                StockUpdate::Order { stock_name, action, quantity } => {
                    if let Some(stock) = stock_data_locked.iter_mut().find(|s| s.name == stock_name) {
                        match action.as_str() {
                            "Buy" => {
                                if stock.availability >= quantity {
                                    stock.availability -= quantity;
                                    stock.price += stock.price * 0.05;

                                    // Print stock details after Buy
                                    println!(
                                        "[Order Processed: Buy] Stock: {}, New Price: {:.2}, New Availability: {}",
                                        stock.name, stock.price, stock.availability
                                    );
                                    println!("--------------------------------------------------------------------------");
                                } else {
                                    println!(
                                        "[Order Rejected: Buy] Insufficient availability for {}: Requested {}, Available {}",
                                        stock.name, quantity, stock.availability
                                    );
                                }
                            }
                            "Sell" => {
                                stock.availability += quantity;
                                stock.price = (stock.price - stock.price * 0.05).max(1.0);

                                // Print stock details after Sell
                                println!(
                                        "[Order Processed: Sell] Stock: {}, New Price: {:.2}, New Availability: {}\n",
                                        stock.name, stock.price, stock.availability
                                    );
                                println!("--------------------------------------------------------------------------");
                            }
                            _ => println!("[Order Error] Unknown action: {}", action),
                        }
                    } else {
                        println!("[Order Error] Stock not found: {}", stock_name);
                    }
                }
            }
        }
    });
}

/// Start the random event trigger thread
fn start_random_event_trigger(sender: mpsc::Sender<StockUpdate>) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(25)); // Trigger random events every 20 seconds

        let mut rng = rand::thread_rng();
        let events = vec![
            ("US Election", 0.2),
            ("Interest Rate Hike", -0.1),
            ("Economic Boom", 0.15),
            ("Pandemic News", -0.2),
        ];
        let (event_name, impact) = events[rng.gen_range(0..events.len())];

        sender.send(StockUpdate::RandomEvent {
            event_name: event_name.to_string(),
            impact,
        }).expect("Failed to send random event");
    });
}

/// Function to run the market timer
fn run_market_timer(start_time: Instant, shutdown_time: Duration) {
    while Instant::now() - start_time < shutdown_time {
        thread::sleep(Duration::from_secs(1));
    }
}


/// Start the price fluctuation thread
fn start_price_fluctuator(sender: mpsc::Sender<StockUpdate>) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(5)); // Trigger fluctuations every 5 seconds

        let stocks = initialize_stocks(); // Replace with actual shared stock logic if needed
        for stock in stocks {
            let fluctuation = (rand::random::<f64>() - 0.5) * 0.2;   //fluctuation between -10% and +10%
            sender.send(StockUpdate::PriceFluctuation {
                stock_name: stock.name.clone(),
                fluctuation,
            }).expect("Failed to send price fluctuation");
        }
    });
}