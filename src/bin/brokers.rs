use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::collections::HashMap;

// Struct for Order
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Order {
    pub order_id: u32,
    pub stock: String,
    pub action: String, // "Buy" or "Sell"
    pub quantity: u32,
    pub price: f64,     // For market orders, price = 0
    pub order_type: String, // "Market" or "Limit"
}

// Struct for Broker
pub struct Broker {
    pub id: u32,
    pub orders: Vec<Order>,          // Holds orders assigned to the broker
    pub sender: mpsc::Sender<Order>, // Sender to communicate with the stock system
    pub stock_prices: Arc<Mutex<HashMap<String, f64>>>, // Shared stock prices
}

impl Broker {
    // Create a new Broker
    pub fn new(
        id: u32,
        sender: mpsc::Sender<Order>,
        stock_prices: Arc<Mutex<HashMap<String, f64>>>,
    ) -> Self {
        Self {
            id,
            orders: Vec::new(),
            sender,
            stock_prices,
        }
    }

    pub fn handle_order(&mut self, order: Order) {
        match order.order_type.as_str() {
            "Market" => {
                println!(
                    "[Broker] Processing Market Order: Stock: {}, Action: {}, Quantity: {}, Price: {:.2}",
                    order.stock, order.action, order.quantity, order.price
                );
                self.process_market_order(order);
            }
            "Limit" => {
                println!(
                    "[Broker] Processing Limit Order: Stock: {}, Action: {}, Quantity: {}, Limit Price: {:.2}",
                    order.stock, order.action, order.quantity, order.price
                );
                self.process_limit_order(order);
            }
            _ => println!("[Broker] Unknown order type: {:?}", order),
        }
    }    

    fn process_market_order(&self, mut order: Order) {
        // Fetch the current market price
        let current_price = {
            let prices = self.stock_prices.lock().unwrap();
            *prices.get(&order.stock).unwrap_or(&0.0)
        };
    
        // Set the current market price
        order.price = current_price;
    
        println!("-----------------------------[Market Order]-------------------------------\n");
        println!(
            "[Market Order Executed] Stock: {}, Action: {}, Quantity: {}, Price: {:.2}\n",
            order.stock, order.action, order.quantity, order.price
        );
    
        // Clone the order before sending
        let order_to_send = order.clone();
    
        // Send the market order to the stock system
        self.sender
            .send(order_to_send)
            .expect("Failed to send market order to stock system");
    
        println!(
            "[Stock System] Market Order Sent: Stock: {}, Action: {}, Quantity: {}, Price: {:.2}, Type: Market\n",
            order.stock, order.action, order.quantity, order.price
        );
    }
    
    fn process_limit_order(&self, order: Order) {
        let stock_name = order.stock.clone();
        let action = order.action.clone();
        let limit_price = order.price;
        let stock_prices = Arc::clone(&self.stock_prices);
        let sender = self.sender.clone();
    
        println!("\n--------------------------------------------------------------------------\n");
        println!(
            "[Limit Order Received]\n  Stock: {}\n  Action: {}\n  Quantity: {}\n  Limit Price: {:.2}",
            stock_name, action, order.quantity, limit_price
        );
    
        thread::spawn(move || {
            loop {
                // Check the current stock price
                let current_price = {
                    let prices = stock_prices.lock().unwrap();
                    *prices.get(&stock_name).unwrap_or(&0.0)
                };
    
                // Check if the condition is met
                let condition_met = match action.as_str() {
                    "Buy" => current_price <= limit_price, // Buy if price <= limit
                    "Sell" => current_price >= limit_price, // Sell if price >= limit
                    _ => false,
                };
    
                if condition_met {
                    println!(
                        "[Limit Order Executed]\n  Stock: {}\n  Action: {}\n  Quantity: {}\n  Price: {:.2} | Limit: {:.2}",
                        stock_name, action, order.quantity, current_price, limit_price
                    );
    
                    // Send the order to the stock system
                    sender
                        .send(order.clone())
                        .expect("Failed to send limit order to stock system");
    
                    println!(
                        "[Stock System] Order Sent: Stock: {}, Action: {}, Quantity: {}, Price: {:.2}, Type: Limit\n--------------------------------------------------------------------------\n",
                        stock_name, action, order.quantity, current_price
                    );
                    break;
                }
    
                // Sleep before rechecking
                thread::sleep(Duration::from_secs(2));
            }
        });
    }    
}