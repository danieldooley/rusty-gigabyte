pub struct MMU {
    // Following: http://imrannazar.com/GameBoy-Emulation-in-JavaScript:-Memory

    /*
        A flag indicating whether the bios is mapped in.

        When the GB starts up the bios is available in memory region 0x0000-0x00FF. Once the
        bios has been run this region is mapped to the cartridge.
     */
    in_bios: bool,

    bios: [u8; (0x00FF - 0x0000) + 1], //using this notation to mean addresses 0x0000 -> 0x00FF

    rom_bank0: [u8; (0x3FFF - 0x0000) + 1],
    // Bank 0 of the cartridge, this is always available
    rom_bankx: [u8; (0x7FFF - 0x4000) + 1], // The cartridge can contain extra banks that are swapped out with a chip on the cartridge

    g_ram: [u8; (0x9FFF - 0x4000) + 1], // Data for programs and sprites is stored here

    e_ram: [u8; (0xBFFF - 0xA000) + 1], // Extra (external) ram that may be present on the cartridge

    w_ram: [u8; (0xDFFF - 0xC000) + 1], // Working ram on the GB
    // Working ram is also available 0xE000-0xFDFF as a shadow copy (due to wiring of the GB) (except the last 512 bytes)

    s_info: [u8; (0xFE9F - 0xFE00) + 1],// Information about the sprites current rendered by the graphics chip

    mm_io: [u8; (0xFF7F - 0xFF00) + 1], // Memory mapped IO (control values used by sound, graphics etc)

    z_ram: [u8; (0xFFFF - 0xFF80) + 1], // "Page Zero" - high speed RAM
}

pub fn new_mmu() -> MMU {
    MMU {
        in_bios: false,
        bios: [0; 256],
        rom_bank0: [0; 16384],
        rom_bankx: [0; 16384],
        g_ram: [0; 24576],
        e_ram: [0; 8192],
        w_ram: [0; 8192],
        s_info: [0; 160],
        mm_io: [0; 128],
        z_ram: [0; 128],
    }
}

impl MMU {
    /*
        Read byte
     */
    pub fn rb(&mut self, addr: u16) -> u8 {
        match addr & 0xF000 {
            0x0000 => {
                if self.in_bios {
                    if addr < 0x0100 {
                        return self.bios[addr as usize];
                    } else if addr == 0x0100 {
                        self.in_bios = false;
                    }
                }

                self.rom_bank0[addr as usize]
            }
            0x1000 | 0x2000 | 0x3000 => {
                self.rom_bank0[addr as usize]
            }
            0x4000 | 0x5000 | 0x6000 | 0x7000 => {
                self.rom_bankx[addr as usize - 0x4000]
            }
            0x8000 | 0x9000 => {
                self.g_ram[addr as usize - 0x8000]
            }
            0xA000 | 0xB000 => {
                self.e_ram[addr as usize - 0xA000]
            }
            0xC000 | 0xD000 => {
                self.w_ram[addr as usize - 0xC000]
            }
            0xE000 => {
                self.w_ram[addr as usize - 0xE000]
            }
            0xF000 => {
                match addr & 0x0F00 {
                    0x0000..=0x0D00 => {
                        self.w_ram[addr as usize - 0xF000]
                    }
                    0x0E00 => {
                        if addr < 0xFEA0 {
                            return self.s_info[addr as usize - 0xFEFF];
                        }

                        0 // Only 160 bytes should actually be addressable
                    }
                    0x0F00 => {
                        if addr < 0xFF80 {
                            return 0; // TODO: Implement IO?
                        }

                        self.z_ram[addr as usize - 0xFF80]
                    }
                    _ => panic!("unmapped memory at {:#06x}", addr)
                }
            }
            _ => panic!("unmapped memory at {:#06x}", addr) // I'm pretty sure these won't happen, rust just isn't able to determine that the above is exhaustive
        }
    }

    /*
        Read word
     */
    pub fn rw(&mut self, addr: u16) -> u16 {
        self.rb(addr) as u16 + ((self.rb(addr) as u16) << 8)
    }

    /*
        Write byte
     */
    pub fn wb(&mut self, addr: u16, val: u8) {
        match addr & 0xF000 {
            0x0000 => {
                // All ROM
            }
            0x1000 | 0x2000 | 0x3000 => {
                // All ROM
            }
            0x4000 | 0x5000 | 0x6000 | 0x7000 => {
                // All ROM
                // TODO: Some of this, or bank 0 might be writable with MBC (bank switching)
            }
            0x8000 | 0x9000 => {
                self.g_ram[addr as usize - 0x8000] = val
            }
            0xA000 | 0xB000 => {
                self.e_ram[addr as usize - 0xA000] = val
            }
            0xC000 | 0xD000 => {
                self.w_ram[addr as usize - 0xC000] = val
            }
            0xE000 => {
                self.w_ram[addr as usize - 0xE000] = val
            }
            0xF000 => {
                match addr & 0x0F00 {
                    0x0000..=0x0D00 => {
                        self.w_ram[addr as usize - 0xF000] = val
                    }
                    0x0E00 => {
                        if addr < 0xFEA0 {
                            self.s_info[addr as usize - 0xFEFF] = val
                        }

                        // Only 160 bytes should actually be addressable
                    }
                    0x0F00 => {
                        if addr < 0xFF80 {
                            // TODO: Implement IO?
                        }

                        self.z_ram[addr as usize - 0xFF80] = val
                    }
                    _ => panic!("unmapped memory at {:#06x}", addr)
                }
            }
            _ => panic!("unmapped memory at {:#06x}", addr) // I'm pretty sure these won't happen, rust just isn't able to determine that the above is exhaustive
        }
    }

    /*
        Write word
     */
    pub fn ww(&mut self, addr: u16, val: u16) {
        self.wb(addr, val as u8);
        self.wb(addr + 1, (val >> 8) as u8)
    }
}