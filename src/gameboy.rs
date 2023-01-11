use std::sync::mpsc::Sender;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use crate::gameboy::cartridge::Cartridge;
use crate::gameboy::cpu::new_cpu;
use crate::gameboy::gpu::new_gpu;
use crate::gameboy::mmu::new_mmu;

pub mod cartridge;
mod cpu;
mod mmu;
mod gpu;

pub fn start_game_boy(cart: Cartridge, image_sender: Sender<Vec<u8>>) {
    let mut mmu = new_mmu(cart);

    let mut cpu = new_cpu();
    let mut gpu = new_gpu(image_sender);

    let target_frame_time = Duration::from_millis(1000 / 60);

    loop {
        let mut fclk = 70224; // 70224 cycles per frame
        let start = SystemTime::now();

        while fclk > 0 {
            /*
            Originally I wrote the CPU to contain MMU when it was constructed.

            Of course, both the CPU and GPU need access to the MMU. On top of this
            the CPU needs to mutate the MMU. This means that with the base ownership/borrowing
            rules MMU cannot exist in both the CPU and GPU.

            As these are both accessing the MMU in a single thread there's no actual risk of a data
            race. However I chose (as I'm learning) to pass the MMU into each execution of the CPU
            and GPU. This guarantees that only one of these has a mutable reference to the MMU at
            a time.

            Another solution would be to give each an Rc<MMU> and then change the mutable fields
            of MMU to be wrapped with RefCell<T>.
                - Rc<MMU> would allow each of CPU and GPU to maintain a reference to the same MMU.
                  However, Rc doesn't allow mutability.
                - RefCell<T> allows implementing "Interior Mutability". I understand this to mean
                  that an immutable MMU would be allowed to modify its own interior values.

            However; the RefCell<T> still enforces borrowing rules, except at runtime rather than
            compile time. This means that if the code in the future is refactored it may compile
            but actually contain the possibility of panicking.
         */
            let (_, delta_t) = cpu.exec(&mut mmu);
            gpu.step(&mut mmu, delta_t);

            fclk -= delta_t as i32;
        }

        let frame_time = SystemTime::now().duration_since(start).unwrap();

        if frame_time < target_frame_time {
            sleep(target_frame_time - frame_time)
        } else if !mmu::DEBUG_GB_DOCTOR {
            eprintln!("slow frame: {}ms", frame_time.as_millis())
        }
    }
}