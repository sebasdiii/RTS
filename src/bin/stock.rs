mod stock_data;

use amiquip::{Connection, Exchange, Publish};
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

