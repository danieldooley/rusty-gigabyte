mod cartridge;
mod cpu;
mod mmu;

use crate::mmu::new_mmu;
use crate::cpu::new_cpu;

fn main() {
    println!("Initialising CPU");

    let mmu = new_mmu();

    let mut cpu = new_cpu(mmu);

    cpu.exec();
}
