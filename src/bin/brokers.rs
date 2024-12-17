// use serde::{Deserialize, Serialize};
// use std::sync::{mpsc, Arc, Mutex};
// use std::thread;
// use std::time::Duration;

// // Struct for Order
// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct Order {
//     pub order_id: u32,
//     pub stock: String,
//     pub action: String, // "Buy" or "Sell"
//     pub quantity: u32,
//     pub price: f64,
// }

// // Struct for Broker
// pub struct Broker {
//     pub id: u32,
//     pub orders: Vec<Order>, // Holds orders assigned to the broker
//     pub sender: mpsc::Sender<Order>, // Sender to communicate with the stock system
// }

// impl Broker {
//     // Create a new Broker
//     pub fn new(id: u32, sender: mpsc::Sender<Order>) -> Self {
//         Self {
//             id,
//             orders: Vec::new(),
//             sender,
//         }
//     }

//     // Add an order to the broker
//     pub fn handle_order(&mut self, order: Order) {
//         self.orders.push(order);

//         // If the broker has 3 orders, send them to the stock system
//         if self.orders.len() == 3 {
//             println!("[Broker {}] Sending orders to stock system: {:?}", self.id, self.orders);

//             for order in self.orders.drain(..) {
//                 self.sender.send(order).expect("Failed to send order to stock system");
//             }
//         }
//     }
// }

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

    // Handle an incoming order from the client
    pub fn handle_order(&mut self, order: Order) {
        match order.order_type.as_str() {
            "Market" => {
                println!(
                    "[Broker {}] Processing Market Order: {:?}",
                    self.id, order
                );
                self.process_market_order(order);
            }
            "Limit" => {
                println!(
                    "[Broker {}] Processing Limit Order: {:?}",
                    self.id, order
                );
                self.process_limit_order(order);
            }
            _ => {
                println!(
                    "[Broker {}] Unknown order type: {:?}",
                    self.id, order
                );
            }
        }
    }

    fn process_market_order(&self, mut order: Order) {
        // Fetch the current market price for the stock
        let current_price = {
            let prices = self.stock_prices.lock().unwrap();
            *prices.get(&order.stock).unwrap_or(&0.0) // Default to 0.0 if not found
        };
    
        // Update the order's price to the current market price
        order.price = current_price;
    
        println!(
            "[Market Order] Broker {} executed order:\n  Stock: {}\n  Action: {}\n  Quantity: {}\n  Price: {:.2}\n",
            self.id, order.stock, order.action, order.quantity, order.price
        );
    
        // Send the market order to the stock system
        self.sender
            .send(order)
            .expect("Failed to send market order to stock system");
    }

    fn process_limit_order(&self, order: Order) {
        if order.price <= 0.0 {
            println!("[Limit Order] Invalid order price: {:?}", order);
            return;
        }
    
        let stock_name = order.stock.clone();
        let action = order.action.clone();
        let limit_price = order.price;
        let stock_prices = Arc::clone(&self.stock_prices);
        let sender = self.sender.clone();
    
        println!(
            "[Limit Order] Broker {} received order:\n  Stock: {}\n  Action: {}\n  Quantity: {}\n  Limit Price: {:.2}\n",
            self.id, order.stock, order.action, order.quantity, order.price
        );
    
        thread::spawn(move || {
            loop {
                let current_price = {
                    let prices = stock_prices.lock().unwrap();
                    *prices.get(&stock_name).unwrap_or(&0.0)
                };
    
                // Check if the limit condition is met
                let condition_met = match action.as_str() {
                    "Buy" => current_price <= limit_price, // Buy if price is <= limit
                    "Sell" => current_price >= limit_price, // Sell if price is >= limit
                    _ => false,
                };
    
                if condition_met {
                    println!(
                        "[Limit Order Executed] Stock: {} | Action: {} | Quantity: {} | Price: {:.2} | Limit: {:.2}",
                        stock_name, action, order.quantity, current_price, limit_price
                    );
    
                    // Send the order to the stock system
                    sender
                        .send(order.clone())
                        .expect("Failed to send limit order to stock system");
    
                    break;
                }
    
                // Wait before rechecking the price
                thread::sleep(Duration::from_secs(2));
            }
        });
    }    
}
