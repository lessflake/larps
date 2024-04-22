#![windows_subsystem = "console"]

use std::{
    process,
    sync::{mpsc, Arc},
    thread,
};

use larps::{
    capture,
    meter::{Data, Meter},
    ui,
};

fn main() -> anyhow::Result<()> {
    let (ctx_oneshot_tx, ctx_oneshot_rx) = mpsc::channel();
    let data = Data::new();
    start_capture(ctx_oneshot_rx, Arc::clone(&data));
    ui::run(ctx_oneshot_tx, data, 8)
}

fn start_capture(ctx_rx: mpsc::Receiver<egui::Context>, data: Arc<parking_lot::Mutex<Data>>) {
    thread::spawn(move || {
        let ctx = ctx_rx.recv().expect("egui context channel closed");
        let meter = Meter::new(ctx, data).expect("meter init failed -- missing resources?");
        if let Err(e) = capture::run(meter) {
            println!("backend: {:?}\nclosing", e);
            process::exit(1);
        }
    });
}
