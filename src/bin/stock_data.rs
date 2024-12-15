use rand::random;

#[derive(Debug, Clone)]
pub struct Stock {
    pub name: String,
    pub price: f64,
    pub availability: u32, // Number of shares available
}

impl Stock {
    // Simulate price fluctuation for the stock
    pub fn fluctuate_price(&mut self) {
        let change = (random::<f64>() - 0.5) * 10.0; // Random fluctuation [-5, +5]
        self.price = (self.price + change).max(1.0); // Ensure price stays positive
    }
}

// Function to initialize stock data
pub fn initialize_stocks() -> Vec<Stock> {
    vec![
        Stock { name: "AAPL".to_string(), price: 150.0, availability: 1000 },
        Stock { name: "GOOGL".to_string(), price: 2800.0, availability: 800 },
        Stock { name: "AMZN".to_string(), price: 3400.0, availability: 600 },
        Stock { name: "TSLA".to_string(), price: 700.0, availability: 1200 },
        Stock { name: "MSFT".to_string(), price: 290.0, availability: 900 },
    ]
}
