extern crate core;

use std::sync::Arc;
use std::thread;

use speedy2d::dimen::Vector2;
use speedy2d::Window;
use speedy2d::window::{WindowCreationOptions, WindowSize};

use gameboy::cartridge::new_cartridge_from_file;
use gameboy::start_game_boy;
use crate::gameboy::cartridge::new_cartridge_from_url;

use crate::gameboy::keys::new_key_reg;
use crate::window::{new_gb_window_handler};

mod window;
mod gameboy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    /*
        cpu_instrs test status
        - 01-special.gb - PASSED
        - 02-interrupts.gb - TODO: FAILED
        - 03-op sp,hl.gb - PASSED
        - 04-op r,imm.gb - PASSED
        - 05-op rp.gb - PASSED
        - 06-ld r,r.gb - PASSED
        - 07-jr,jp,call,ret,rst.gb - PASSED - Currently master of gameboy doctor is wrong: https://github.com/robert/gameboy-doctor/pull/8
        - 08-misc instrs.gb - PASSED
        - 09-op r,r.gb - PASSED
        - 10-bit ops.gb - PASSED
        - 11-op a,(hl).gb - PASSED
     */

    // let cart = new_cartridge_from_file("roms/cpu_instrs/individual/02-interrupts.gb")?;
    // let cart = new_cartridge_from_file("roms/ttt.gb")?;

    let cart = new_cartridge_from_url("http://imrannazar.com/stuff/software/jsgb/tests/tetris.gb")?;

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
