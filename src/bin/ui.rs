use eframe::egui;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Struct to hold stock data
#[derive(Clone)]
pub struct StockUpdate {
    pub stock: String,
    pub price: f64,
    pub availability: u32,
}

// UI Application
pub struct StockApp {
    updates: Arc<Mutex<Vec<StockUpdate>>>,
}

impl StockApp {
    pub fn new(updates: Arc<Mutex<Vec<StockUpdate>>>) -> Self {
        Self { updates }
    }
}

impl eframe::App for StockApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Stock Updates");
            ui.separator();

            // Display stock updates
            let updates = self.updates.lock().unwrap();
            for stock in updates.iter() {
                ui.horizontal(|ui| {
                    ui.label(format!("Stock: {}", stock.stock));
                    ui.label(format!("Price: {:.2}", stock.price));
                    ui.label(format!("Availability: {}", stock.availability));
                });
                ui.separator();
            }
        });

        // Refresh the UI periodically
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}
