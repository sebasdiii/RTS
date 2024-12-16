use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

// Struct for Order
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Order {
    pub order_id: u32,
    pub stock: String,
    pub action: String, // "Buy" or "Sell"
    pub quantity: u32,
    pub price: f64,
}

// Struct for Broker
pub struct Broker {
    pub id: u32,
    pub orders: Vec<Order>, // Holds orders assigned to the broker
    pub sender: mpsc::Sender<Order>, // Sender to communicate with the stock system
}

impl Broker {
    // Create a new Broker
    pub fn new(id: u32, sender: mpsc::Sender<Order>) -> Self {
        Self {
            id,
            orders: Vec::new(),
            sender,
        }
    }

    // Add an order to the broker
    pub fn handle_order(&mut self, order: Order) {
        self.orders.push(order);

        // If the broker has 3 orders, send them to the stock system
        if self.orders.len() == 3 {
            println!("[Broker {}] Sending orders to stock system: {:?}", self.id, self.orders);

            for order in self.orders.drain(..) {
                self.sender.send(order).expect("Failed to send order to stock system");
            }
        }
    }
}
