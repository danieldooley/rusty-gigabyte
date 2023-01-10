use std::thread;

use gameboy::cartridge::new_cartridge_from_file;
use gameboy::start_game_boy;
use speedy2d::Window;

use crate::window::GBWindowHandler;

mod window;
mod gameboy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Initialising CPU");

    let cart = new_cartridge_from_file("roms/cpu_instrs/cpu_instrs.gb")?;

    // spawn a thread for the gameboy
    thread::spawn(move || {
        start_game_boy(cart);
    });

    // Window needs to run on the main thread.
    let window = Window::<()>::new_centered("Rusty GB", (160, 144))?;

    window.run_loop(GBWindowHandler {});

    Ok(())
}
