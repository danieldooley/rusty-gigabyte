extern crate core;

use std::sync::Arc;
use std::thread;

use speedy2d::dimen::Vector2;
use speedy2d::Window;
use speedy2d::window::{WindowCreationOptions, WindowSize};

use gameboy::cartridge::new_cartridge_from_file;
use gameboy::start_game_boy;

use crate::gameboy::keys::new_key_reg;
use crate::window::{new_gb_window_handler};

mod window;
mod gameboy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let cart = new_cartridge_from_file("roms/cpu_instrs/cpu_instrs.gb")?;
    let cart = new_cartridge_from_file("roms/ttt.gb")?;

    let key_reg = Arc::new(new_key_reg());

    let key_reg_clone = key_reg.clone();

    let window = Window::<Vec<u8>>::new_with_user_events("Rusty GB", WindowCreationOptions::new_windowed(WindowSize::ScaledPixels(Vector2::from((160.0, 144.0))), None))?;

    // Window needs to run on the main thread.
    let image_sender = window.create_user_event_sender();

    // spawn a thread for the gameboy
    thread::spawn(move || {
        start_game_boy(cart, image_sender, key_reg_clone);
    });

    window.run_loop(new_gb_window_handler(key_reg));

    Ok(())
}
