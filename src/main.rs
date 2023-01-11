extern crate core;

use std::sync::mpsc::channel;
use std::thread;

use gameboy::cartridge::new_cartridge_from_file;
use gameboy::start_game_boy;
use speedy2d::Window;

use crate::window::{GBWindowHandler, new_gb_window_handler};

mod window;
mod gameboy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cart = new_cartridge_from_file("roms/cpu_instrs/individual/11-op a,(hl).gb")?;

    let (image_sender, image_receiver) = channel();

    // spawn a thread for the gameboy
    thread::spawn(move || {
        start_game_boy(cart, image_sender);
    });

    // Window needs to run on the main thread.
    let window = Window::<()>::new_centered("Rusty GB", (1280, 1152))?;

    window.run_loop(new_gb_window_handler(image_receiver));

    Ok(())
}
