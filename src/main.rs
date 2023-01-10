use crate::cpu::new_cpu;
use crate::mmu::new_mmu;

mod cartridge;
mod cpu;
mod mmu;

fn main() {
    println!("Initialising CPU");

    let mmu = new_mmu();

    let mut cpu = new_cpu(mmu);

    cpu.exec();
}
