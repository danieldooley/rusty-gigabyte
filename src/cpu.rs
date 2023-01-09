use std::ops::Add;

use crate::mmu::MMU;

/*
    Conventions used (from: https://rgbds.gbdev.io/docs/v0.6.0/gbz80.7):
    - r8: any 8 bit register (a, b, c, d, e, h, l)
    - r16: any 16 bit register (bc, de, hl)
    - n8: 8 bit integer constant
    - n16: 16 bit integer constant
    - e8: 8 bit integer offset (-128 to 127)
    - u3: 3 bit unsigned integer constant (0 to 7)
    - cc: Condition code:
        - Z: execute if Z is set
        - NZ: execute if Z is not set
        - C: execute if C is set
        - NC: execute if C is not set
        - !cc: negates a condition code
    - vec: One of the RST vectors: (0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, and 0x38)

    My conventions:
    - Register prefixed with `m` uses it as a memory address
 */

enum R8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

enum R16 {
    BC,
    DE,
    HL,
}

enum CC {
    Z,
    NZ,
    C,
    NC,
}

enum RST {
    // The call reference at RGBDS only defines RST00..RST38 but the code at http://imrannazar.com/content/files/jsgb.z80.js goes to RST60
    RST00,
    RST08,
    RST10,
    RST18,
    RST20,
    RST28,
    RST30,
    RST38,
    RST40,
    RST48,
    RST50,
    RST58,
    RST60,
}

impl Into<u16> for RST {
    fn into(self) -> u16 {
        match self {
            RST::RST00 => 0x00,
            RST::RST08 => 0x08,
            RST::RST10 => 0x10,
            RST::RST18 => 0x18,
            RST::RST20 => 0x20,
            RST::RST28 => 0x28,
            RST::RST30 => 0x30,
            RST::RST38 => 0x38,
            RST::RST40 => 0x40,
            RST::RST48 => 0x48,
            RST::RST50 => 0x50,
            RST::RST58 => 0x58,
            RST::RST60 => 0x60,
        }
    }
}

const FLAG_ZERO: u8 = 0x80;
const FLAG_SUB: u8 = 0x40;
const FLAG_HALF_CARRY: u8 = 0x20;
const FLAG_CARRY: u8 = 0x10;

pub struct CPU {
    // clocks
    clock_m: u8,
    clock_t: u8,

    // 8 bit registers
    reg_a: u8,
    reg_b: u8,
    reg_c: u8,
    reg_d: u8,
    reg_e: u8,
    reg_f: u8,
    // f contains flags
    reg_h: u8,
    reg_l: u8,
    // b&c, d&e, h&l can be used as 16bit registers

    // 16 bit registers
    reg_pc: u16,
    reg_sp: u16,

    // clock for last instr
    reg_m: u8,
    // should be t divided by 4
    reg_t: u8, // how many cycles the last operation took
    // timings from https://gbdev.io/pandocs/CPU_Instruction_Set.html

    // Whether interrupts are enabled
    ime: bool,

    // Halt represents a low power mode until an interrupt occurs
    halt: bool,

    // Represents stopped?
    stop: bool,

    // Reference to MMU for accessing memory
    mmu: MMU,
}

pub fn new_cpu(mmu: MMU) -> CPU {
    CPU {
        clock_m: 0,
        clock_t: 0,
        reg_a: 0,
        reg_b: 0,
        reg_c: 0,
        reg_d: 0,
        reg_e: 0,
        reg_f: 0,
        reg_h: 0,
        reg_l: 0,
        reg_pc: 0,
        reg_sp: 0,
        reg_m: 0,
        reg_t: 0,
        ime: false,
        halt: false,
        stop: false,
        mmu,
    }
}

impl CPU {
    /*
        Reset the CPU to its initial state
     */
    pub fn reset(&mut self) {
        self.clock_m = 0;
        self.clock_t = 0;

        self.reg_a = 0;
        self.reg_b = 0;
        self.reg_c = 0;
        self.reg_d = 0;
        self.reg_e = 0;
        self.reg_f = 0;
        self.reg_h = 0;
        self.reg_l = 0;

        self.reg_pc = 0;
        self.reg_sp = 0;

        self.reg_m = 0;
        self.reg_t = 0;

        self.ime = false;
        self.halt = false;
    }

    /*
        #########
        Utilities
        #########
     */
    // fn set_m_cycles(&mut self, cycles: u8) {
    //     self.reg_m = cycles;
    //     self.reg_t = cycles * 4;
    // }

    fn set_flags_u8(&mut self, val: u8, carry: bool, sub: bool) {
        self.reg_f = 0; // Reset flags
        if val == 0 {
            self.reg_f |= FLAG_ZERO
        }

        if carry {
            self.reg_f |= FLAG_CARRY
        }

        if sub {
            self.reg_f |= FLAG_SUB
        }

        // TODO: Half Carries?
    }

    fn set_flags_u16(&mut self, val: u16, carry: bool, sub: bool) {
        self.reg_f = 0; // Reset flags
        if val == 0 {
            self.reg_f |= FLAG_ZERO
        }

        if carry {
            self.reg_f |= FLAG_CARRY
        }

        if sub {
            self.reg_f |= FLAG_SUB
        }

        // TODO: Half Carries?
    }

    /*
        ####################
        Arithmetic and Logic
        ####################
     */

    /*
        Add the value in r8 plus the carry flag to A
     */
    fn adc_a_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        // TODO: In the future could use carrying adds from https://github.com/rust-lang/rust/issues/85532

        let (mut res, mut carry) = self.reg_a.overflowing_add(val);

        if self.reg_f & FLAG_CARRY > 0 {
            let (res2, carry2) = res.overflowing_add(1);
            res = res2;
            carry = carry && carry2;
        }

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, false);

        1
    }

    /*
        Add the value in address HL plus the carry flag to A
    */
    fn adc_a_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let (mut res, mut carry) = self.reg_a.overflowing_add(val);

        if self.reg_f & FLAG_CARRY > 0 {
            let (res2, carry2) = res.overflowing_add(1);
            res = res2;
            carry = carry && carry2;
        }

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, false);

        2
    }

    /*
        Add the constant value n8 plus the carry flag to A
     */
    fn adc_a_n8(&mut self) -> u8 {
        let val = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        let (mut res, mut carry) = self.reg_a.overflowing_add(val);

        if self.reg_f & FLAG_CARRY > 0 {
            let (res2, carry2) = res.overflowing_add(1);
            res = res2;
            carry = carry && carry2;
        }

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, false);

        2
    }

    /*
        Add the value in r8 to A
     */
    fn add_a_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        let (res, carry) = self.reg_a.overflowing_add(val);

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, false);

        1
    }

    /*
        Add the value at address HL to A
     */
    fn add_a_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let (res, carry) = self.reg_a.overflowing_add(val);

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, false);

        2
    }

    /*
        Add the constant value n8 to A
     */
    fn add_a_n8(&mut self) -> u8 {
        let val = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        let (res, carry) = self.reg_a.overflowing_add(val);

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, false);

        2
    }

    /*
        Add the value in R16 to HL
     */
    fn add_hl_r16(&mut self, r: R16) -> u8 {
        let val = match r {
            R16::BC => (self.reg_b as u16) << 8 + (self.reg_c as u16),
            R16::DE => (self.reg_d as u16) << 8 + (self.reg_e as u16),
            R16::HL => (self.reg_h as u16) << 8 + (self.reg_l as u16),
        };

        let (res, carry) = ((self.reg_h as u16) << 8 + (self.reg_l as u16)).overflowing_add(val);

        self.reg_h = ((res as u16) >> 8) as u8;
        self.reg_l = res as u8;

        self.set_flags_u16(res, carry, false);

        2
    }

    /*
        Add the value in SP to HL
     */
    fn add_hl_sp(&mut self) -> u8 {
        let val = self.reg_sp;

        let (res, carry) = ((self.reg_h as u16) << 8 + (self.reg_l as u16)).overflowing_add(val);

        self.reg_h = ((res as u16) >> 8) as u8;
        self.reg_l = res as u8;

        self.set_flags_u16(res, carry, false);

        2
    }

    /*
        Add the signed value e8 to SP
     */
    fn add_sp_e8(&mut self) -> u8 {
        let e8 = self.mmu.rsb(self.reg_pc);
        self.reg_pc += 1;

        let (res, carry) = self.reg_sp.overflowing_add_signed(e8 as i16);

        self.reg_h = ((res as u16) >> 8) as u8;
        self.reg_l = res as u8;

        self.set_flags_u16(res, carry, false);

        4
    }

    /*
        Bitwise AND between the value in r8 and A
     */
    fn and_a_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        self.reg_a &= val;

        self.set_flags_u8(self.reg_a, false, false);

        1
    }

    /*
        Bitwise AND between the value in address HL and A
     */
    fn and_a_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        self.reg_a &= val;

        self.set_flags_u8(self.reg_a, false, false);

        1
    }

    /*
        Bitwise AND between the constant n8 and A
     */
    fn and_a_n8(&mut self) -> u8 {
        let val = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        self.reg_a &= val;

        self.set_flags_u8(self.reg_a, false, false);

        1
    }

    /*
        Subtract the value in R8 from A and set the flags but don't store result
     */
    fn cp_a_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        let (res, carry) = self.reg_a.overflowing_sub(val);

        self.set_flags_u8(res, carry, true);

        1
    }

    /*
        Subtract the value in address HL from A and set the flags but don't store result
     */
    fn cp_a_mhl(&mut self, r: R8) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let (res, carry) = self.reg_a.overflowing_sub(val);

        self.set_flags_u8(res, carry, true);

        1
    }

    /*
        Subtract the constant value n8 from A and set the flags but don't store result
     */
    fn cp_a_n8(&mut self, r: R8) -> u8 {
        let val = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        let (res, carry) = self.reg_a.overflowing_sub(val);

        self.set_flags_u8(res, carry, true);

        1
    }

    /*
        Decrement the value in r8
     */
    fn dec_r8(&mut self, r: R8) -> u8 {
        let res = match r {
            R8::A => {
                self.reg_a = self.reg_a.wrapping_sub(1);
                self.reg_a
            }
            R8::B => {
                self.reg_b = self.reg_b.wrapping_sub(1);
                self.reg_b
            }
            R8::C => {
                self.reg_c = self.reg_c.wrapping_sub(1);
                self.reg_c
            }
            R8::D => {
                self.reg_d = self.reg_d.wrapping_sub(1);
                self.reg_d
            }
            R8::E => {
                self.reg_e = self.reg_e.wrapping_sub(1);
                self.reg_e
            }
            R8::H => {
                self.reg_h = self.reg_h.wrapping_sub(1);
                self.reg_h
            }
            R8::L => {
                self.reg_l = self.reg_l.wrapping_sub(1);
                self.reg_l
            }
        };

        self.set_flags_u8(res, false, true);

        1
    }

    /*
        Decrement the byte at address HL by 1
     */
    fn dec_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let res = val.wrapping_sub(1);

        self.mmu.wb(addr, res);

        self.set_flags_u8(res, false, true);

        3
    }

    /*
        Decrement the value in register r16
     */
    fn dec_r16(&mut self, r: R16) -> u8 {
        let val = match r {
            R16::BC => (self.reg_b as u16) << 8 + (self.reg_c as u16),
            R16::DE => (self.reg_d as u16) << 8 + (self.reg_e as u16),
            R16::HL => (self.reg_h as u16) << 8 + (self.reg_l as u16),
        };

        let res = val.wrapping_sub(1);

        match r {
            R16::BC => {
                self.reg_b = (res >> 8) as u8;
                self.reg_c = res as u8;
            }
            R16::DE => {
                self.reg_h = (res >> 8) as u8;
                self.reg_l = res as u8;
            }
            R16::HL => {
                self.reg_h = (res >> 8) as u8;
                self.reg_l = res as u8;
            }
        }

        2
    }

    /*
        Decrement the value in register SP by 1
     */
    fn dec_sp(&mut self) -> u8 {
        self.reg_sp = self.reg_sp.wrapping_sub(1);

        2
    }

    /*
        Increment the value in r8
     */
    fn inc_r8(&mut self, r: R8) -> u8 {
        let res = match r {
            R8::A => {
                self.reg_a = self.reg_a.wrapping_add(1);
                self.reg_a
            }
            R8::B => {
                self.reg_b = self.reg_b.wrapping_add(1);
                self.reg_b
            }
            R8::C => {
                self.reg_c = self.reg_c.wrapping_add(1);
                self.reg_c
            }
            R8::D => {
                self.reg_d = self.reg_d.wrapping_add(1);
                self.reg_d
            }
            R8::E => {
                self.reg_e = self.reg_e.wrapping_add(1);
                self.reg_e
            }
            R8::H => {
                self.reg_h = self.reg_h.wrapping_add(1);
                self.reg_h
            }
            R8::L => {
                self.reg_l = self.reg_l.wrapping_add(1);
                self.reg_l
            }
        };

        self.set_flags_u8(res, false, false);

        1
    }

    /*
        Increment the byte at address HL by 1
     */
    fn inc_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let res = val.wrapping_add(1);

        self.mmu.wb(addr, res);

        self.set_flags_u8(res, false, false);

        3
    }

    /*
        Increment the value in register r16
     */
    fn inc_r16(&mut self, r: R16) -> u8 {
        let val = match r {
            R16::BC => (self.reg_b as u16) << 8 + (self.reg_c as u16),
            R16::DE => (self.reg_d as u16) << 8 + (self.reg_e as u16),
            R16::HL => (self.reg_h as u16) << 8 + (self.reg_l as u16),
        };

        let res = val.wrapping_add(1);

        match r {
            R16::BC => {
                self.reg_b = (res >> 8) as u8;
                self.reg_c = res as u8;
            }
            R16::DE => {
                self.reg_h = (res >> 8) as u8;
                self.reg_l = res as u8;
            }
            R16::HL => {
                self.reg_h = (res >> 8) as u8;
                self.reg_l = res as u8;
            }
        }

        2
    }

    /*
        Increment the value in register SP by 1
     */
    fn inc_sp(&mut self) -> u8 {
        self.reg_sp = self.reg_sp.wrapping_add(1);

        2
    }

    /*
        Bitwise OR between the value in r8 and A
     */
    fn or_a_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        self.reg_a |= val;

        self.set_flags_u8(self.reg_a, false, false);

        1
    }

    /*
        Bitwise OR between the value in address HL and A
     */
    fn or_a_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        self.reg_a |= val;

        self.set_flags_u8(self.reg_a, false, false);

        1
    }

    /*
        Bitwise OR between the constant n8 and A
     */
    fn or_a_n8(&mut self) -> u8 {
        let val = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        self.reg_a |= val;

        self.set_flags_u8(self.reg_a, false, false);

        1
    }

    /*
        Subtract the value in r8 and the carry flag from A
     */
    fn sdc_a_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        // TODO: In the future could use carrying subs from https://github.com/rust-lang/rust/issues/85532

        let (mut res, mut carry) = self.reg_a.overflowing_sub(val);

        if self.reg_f & FLAG_CARRY > 0 {
            let (res2, carry2) = res.overflowing_sub(1);
            res = res2;
            carry = carry && carry2;
        }

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, true);

        1
    }

    /*
        Subtract the value in address HL and the carry flag from A
    */
    fn sdc_a_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let (mut res, mut carry) = self.reg_a.overflowing_sub(val);

        if self.reg_f & FLAG_CARRY > 0 {
            let (res2, carry2) = res.overflowing_sub(1);
            res = res2;
            carry = carry && carry2;
        }

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, true);

        2
    }

    /*
        Subtract the constant value n8 and the carry flag from A
     */
    fn sdc_a_n8(&mut self) -> u8 {
        let val = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        let (mut res, mut carry) = self.reg_a.overflowing_sub(val);

        if self.reg_f & FLAG_CARRY > 0 {
            let (res2, carry2) = res.overflowing_sub(1);
            res = res2;
            carry = carry && carry2;
        }

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, true);

        2
    }

    /*
        Subtract the value in r8 from A
     */
    fn sub_a_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        let (res, carry) = self.reg_a.overflowing_sub(val);

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, true);

        1
    }

    /*
        Subtract the value at address HL from A
     */
    fn sub_a_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let (res, carry) = self.reg_a.overflowing_sub(val);

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, true);

        2
    }

    /*
        Subtract the constant value n8 from A
     */
    fn sub_a_n8(&mut self) -> u8 {
        let val = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        let (res, carry) = self.reg_a.overflowing_sub(val);

        self.reg_a = res;

        self.set_flags_u8(self.reg_a, carry, true);

        2
    }

    /*
        Bitwise XOR between the value in r8 and A
    */
    fn xor_a_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        self.reg_a ^= val;

        self.set_flags_u8(self.reg_a, false, false);

        1
    }

    /*
        Bitwise XOR between the value in address HL and A
     */
    fn xor_a_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        self.reg_a ^= val;

        self.set_flags_u8(self.reg_a, false, false);

        1
    }

    /*
        Bitwise XOR between the constant n8 and A
     */
    fn xor_a_n8(&mut self) -> u8 {
        let val = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        self.reg_a ^= val;

        self.set_flags_u8(self.reg_a, false, false);

        1
    }

    /*
        ##############
        Bit Operations
        ##############
     */

    /*
        Test the bit u3 in r8
     */
    fn bit_u3_r8(&mut self, u: u8, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        self.set_flags_u8(val & u, false, false);

        2
    }

    /*
        Test the bit u3 in address HL
     */
    fn bit_u3_mhl(&mut self, u: u8) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        self.set_flags_u8(val & u, false, false);

        3
    }

    /*
        Reset the bit u3 in r8
     */
    fn res_u3_r8(&mut self, u: u8, r: R8) -> u8 {
        match r {
            R8::A => self.reg_a &= !u,
            R8::B => self.reg_b &= !u,
            R8::C => self.reg_c &= !u,
            R8::D => self.reg_d &= !u,
            R8::E => self.reg_e &= !u,
            R8::H => self.reg_h &= !u,
            R8::L => self.reg_l &= !u,
        };

        2
    }

    /*
        Reset the bit u3 in address HL
     */
    fn res_u3_mhl(&mut self, u: u8) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let res = val & !u;

        self.mmu.wb(addr, res);

        4
    }

    /*
        Set the bit u3 in r8
     */
    fn set_u3_r8(&mut self, u: u8, r: R8) -> u8 {
        match r {
            R8::A => self.reg_a |= u,
            R8::B => self.reg_b |= u,
            R8::C => self.reg_c |= u,
            R8::D => self.reg_d |= u,
            R8::E => self.reg_e |= u,
            R8::H => self.reg_h |= u,
            R8::L => self.reg_l |= u,
        };

        2
    }

    /*
        Set the bit u3 in address HL
     */
    fn set_u3_mhl(&mut self, u: u8) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let res = val | u;

        self.mmu.wb(addr, res);

        4
    }

    /*
        Swap the upper bits with the lower in register r8
     */
    fn swap_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => {
                self.reg_a = self.reg_a.rotate_left(4);
                self.reg_a
            }
            R8::B => {
                self.reg_b = self.reg_b.rotate_left(4);
                self.reg_b
            }
            R8::C => {
                self.reg_c = self.reg_c.rotate_left(4);
                self.reg_c
            }
            R8::D => {
                self.reg_d = self.reg_d.rotate_left(4);
                self.reg_d
            }
            R8::E => {
                self.reg_e = self.reg_e.rotate_left(4);
                self.reg_e
            }
            R8::H => {
                self.reg_h = self.reg_h.rotate_left(4);
                self.reg_h
            }
            R8::L => {
                self.reg_l = self.reg_l.rotate_left(4);
                self.reg_l
            }
        };

        self.set_flags_u8(val, false, false);

        2
    }

    /*
        Swap the upper bits with the lower in address HL
     */
    fn swap_mhl(&mut self, u: u8) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let res = val.rotate_left(4);

        self.mmu.wb(addr, res);

        self.set_flags_u8(val, false, false);

        4
    }

    /*
        #####################
        Bitshift Instructions
        #####################
     */

    /*
        Rotate r8 left through carry
     */
    fn rl_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        let carry = self.reg_f & FLAG_CARRY > 0;
        let new_carry = val & 0x80 > 0;

        let mut res = val << 1;

        if carry {
            res += 1;
        }

        match r {
            R8::A => self.reg_a = res,
            R8::B => self.reg_b = res,
            R8::C => self.reg_c = res,
            R8::D => self.reg_d = res,
            R8::E => self.reg_e = res,
            R8::H => self.reg_h = res,
            R8::L => self.reg_l = res,
        };

        self.set_flags_u8(res, new_carry, false);

        2
    }

    /*
        Rotate byte in memory address HL left through carry
     */
    fn rl_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let carry = self.reg_f & FLAG_CARRY > 0;
        let new_carry = val & 0x80 > 0;

        let mut res = val << 1;

        if carry {
            res += 1;
        }

        self.mmu.wb(addr, res);

        self.set_flags_u8(res, new_carry, false);

        4
    }

    /*
        Rotate register A left through carry
     */
    fn rla(&mut self) -> u8 {
        let val = self.reg_a;

        let carry = self.reg_f & FLAG_CARRY > 0;
        let new_carry = val & 0x80 > 0;

        let mut res = val << 1;

        if carry {
            res += 1;
        }

        self.reg_a = res;

        self.set_flags_u8(res, new_carry, false);

        1 // TODO: This could reuse rl_r8 except it has a different cycle count
    }

    /*
        Rotate r8 left
     */
    fn rlc_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        let new_carry = val & 0x80 > 0;

        let res = val.rotate_left(1);

        match r {
            R8::A => self.reg_a = res,
            R8::B => self.reg_b = res,
            R8::C => self.reg_c = res,
            R8::D => self.reg_d = res,
            R8::E => self.reg_e = res,
            R8::H => self.reg_h = res,
            R8::L => self.reg_l = res,
        };

        self.set_flags_u8(res, new_carry, false);

        2
    }

    /*
        Rotate byte in memory address HL left
     */
    fn rlc_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let new_carry = val & 0x80 > 0;

        let mut res = val.rotate_left(1);

        self.mmu.wb(addr, res);

        self.set_flags_u8(res, new_carry, false);

        4
    }

    /*
        Rotate register A left
     */
    fn rlca(&mut self) -> u8 {
        let val = self.reg_a;

        let new_carry = val & 0x80 > 0;

        let mut res = val.rotate_left(1);

        self.reg_a = res;

        self.set_flags_u8(res, new_carry, false);

        1 // TODO: This could reuse rl_r8 except it has a different cycle count
    }

    /*
        Rotate r8 right through carry
     */
    fn rr_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        let carry = self.reg_f & FLAG_CARRY > 0;
        let new_carry = val & 1 > 0;

        let mut res = val >> 1;

        if carry {
            res += 0x80;
        }

        match r {
            R8::A => self.reg_a = res,
            R8::B => self.reg_b = res,
            R8::C => self.reg_c = res,
            R8::D => self.reg_d = res,
            R8::E => self.reg_e = res,
            R8::H => self.reg_h = res,
            R8::L => self.reg_l = res,
        };

        self.set_flags_u8(res, new_carry, false);

        2
    }

    /*
        Rotate byte in memory address HL right through carry
     */
    fn rr_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let carry = self.reg_f & FLAG_CARRY > 0;
        let new_carry = val & 1 > 0;

        let mut res = val >> 1;

        if carry {
            res += 0x80;
        }

        self.mmu.wb(addr, res);

        self.set_flags_u8(res, new_carry, false);

        4
    }

    /*
        Rotate register A right through carry
     */
    fn rra(&mut self) -> u8 {
        let val = self.reg_a;

        let carry = self.reg_f & FLAG_CARRY > 0;
        let new_carry = val & 1 > 0;

        let mut res = val >> 1;

        if carry {
            res += 0x80;
        }

        self.reg_a = res;

        self.set_flags_u8(res, new_carry, false);

        1 // TODO: This could reuse rl_r8 except it has a different cycle count
    }

    /*
        Rotate r8 right
     */
    fn rrc_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        let new_carry = val & 1 > 0;

        let res = val.rotate_right(1);

        match r {
            R8::A => self.reg_a = res,
            R8::B => self.reg_b = res,
            R8::C => self.reg_c = res,
            R8::D => self.reg_d = res,
            R8::E => self.reg_e = res,
            R8::H => self.reg_h = res,
            R8::L => self.reg_l = res,
        };

        self.set_flags_u8(res, new_carry, false);

        2
    }

    /*
        Rotate byte in memory address HL right
     */
    fn rrc_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let new_carry = val & 1 > 0;

        let res = val.rotate_right(1);

        self.mmu.wb(addr, res);

        self.set_flags_u8(res, new_carry, false);

        4
    }

    /*
        Rotate register A right
     */
    fn rrca(&mut self) -> u8 {
        let val = self.reg_a;

        let new_carry = val & 1 > 0;

        let res = val.rotate_right(1);

        self.reg_a = res;

        self.set_flags_u8(res, new_carry, false);

        1 // TODO: This could reuse rl_r8 except it has a different cycle count
    }

    /*
        Shift r8 left arithmetically
     */
    fn sla_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        let new_carry = val & 0x80 > 0;

        let res = val << 1;

        match r {
            R8::A => self.reg_a = res,
            R8::B => self.reg_b = res,
            R8::C => self.reg_c = res,
            R8::D => self.reg_d = res,
            R8::E => self.reg_e = res,
            R8::H => self.reg_h = res,
            R8::L => self.reg_l = res,
        };

        self.set_flags_u8(res, new_carry, false);

        2
    }

    /*
        Shift byte in memory address HL left arithmetically
     */
    fn sla_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let new_carry = val & 0x80 > 0;

        let res = val << 1;

        self.mmu.wb(addr, res);

        self.set_flags_u8(res, new_carry, false);

        4
    }

    /*
        Shift r8 right arithmetically
     */
    fn sra_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        let new_carry = val & 1 > 0;

        let res = (val >> 1) + 0x80;

        match r {
            R8::A => self.reg_a = res,
            R8::B => self.reg_b = res,
            R8::C => self.reg_c = res,
            R8::D => self.reg_d = res,
            R8::E => self.reg_e = res,
            R8::H => self.reg_h = res,
            R8::L => self.reg_l = res,
        };

        self.set_flags_u8(res, new_carry, false);

        2
    }

    /*
        Shift byte in memory address HL right arithmetically
     */
    fn sra_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let new_carry = val & 1 > 0;

        let res = (val >> 1) + 0x80;

        self.mmu.wb(addr, res);

        self.set_flags_u8(res, new_carry, false);

        4
    }

    /*
        Shift r8 right logically
     */
    fn srl_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        let new_carry = val & 1 > 0;

        let res = val >> 1;

        match r {
            R8::A => self.reg_a = res,
            R8::B => self.reg_b = res,
            R8::C => self.reg_c = res,
            R8::D => self.reg_d = res,
            R8::E => self.reg_e = res,
            R8::H => self.reg_h = res,
            R8::L => self.reg_l = res,
        };

        self.set_flags_u8(res, new_carry, false);

        2
    }

    /*
        Shift byte in memory address HL right logically
     */
    fn srl_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let new_carry = val & 1 > 0;

        let res = val >> 1;

        self.mmu.wb(addr, res);

        self.set_flags_u8(res, new_carry, false);

        4
    }

    /*
        ###############
        Load operations
        ###############
     */

    /*
        Load right register into left register
     */
    fn ld_r8_r8(&mut self, r_left: R8, r_right: R8) -> u8 {
        let val = match r_right {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        match r_left {
            R8::A => self.reg_a = val,
            R8::B => self.reg_b = val,
            R8::C => self.reg_c = val,
            R8::D => self.reg_d = val,
            R8::E => self.reg_e = val,
            R8::H => self.reg_h = val,
            R8::L => self.reg_l = val,
        }

        1
    }

    /*
        Load constant n8 into register r8
     */
    fn ld_r8_n8(&mut self, r: R8) -> u8 {
        /*
            Load the value directly from memory
         */
        let val = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        match r {
            R8::A => self.reg_a = val,
            R8::B => self.reg_b = val,
            R8::C => self.reg_c = val,
            R8::D => self.reg_d = val,
            R8::E => self.reg_e = val,
            R8::H => self.reg_h = val,
            R8::L => self.reg_l = val,
        }

        2
    }

    /*
        Load constant n16 into register r16
     */
    fn ld_r16_n16(&mut self, r: R16) -> u8 {
        let upper = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        let lower = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        match r {
            R16::BC => {
                self.reg_b = lower; // TODO: Not sure upper/lower are right here
                self.reg_c = upper;
            }
            R16::DE => {
                self.reg_d = lower;
                self.reg_e = upper;
            }
            R16::HL => {
                self.reg_h = lower;
                self.reg_l = upper;
            }
        }

        3
    }

    /*
        Load value from r8 into address HL
    */
    fn ld_mhl_r8(&mut self, r: R8) -> u8 {
        let val = match r {
            R8::A => self.reg_a,
            R8::B => self.reg_b,
            R8::C => self.reg_c,
            R8::D => self.reg_d,
            R8::E => self.reg_e,
            R8::H => self.reg_h,
            R8::L => self.reg_l,
        };

        self.mmu.wb((self.reg_h as u16) << 8 + (self.reg_l as u16), val);

        2
    }

    /*
        Load constant n8 into address HL
     */
    fn ld_mhl_n8(&mut self) -> u8 {
        let val = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        self.mmu.wb((self.reg_h as u16) << 8 + (self.reg_l as u16), val);

        3
    }

    /*
        Load value at address HL into r8
     */
    fn ld_r8_mhl(&mut self, r: R8) -> u8 {
        let val = self.mmu.rb((self.reg_h as u16) << 8 + (self.reg_l as u16));

        match r {
            R8::A => self.reg_a = val,
            R8::B => self.reg_b = val,
            R8::C => self.reg_c = val,
            R8::D => self.reg_d = val,
            R8::E => self.reg_e = val,
            R8::H => self.reg_h = val,
            R8::L => self.reg_l = val,
        }

        2
    }

    /*
        Load value from A into address pointed to by r16
     */
    fn ld_mr16_a(&mut self, r: R16) -> u8 {
        let val = self.reg_a;

        let addr = match r {
            R16::BC => (self.reg_b as u16) << 8 + (self.reg_c as u16),
            R16::DE => (self.reg_d as u16) << 8 + (self.reg_e as u16),
            R16::HL => (self.reg_h as u16) << 8 + (self.reg_l as u16)
        };

        self.mmu.wb(addr, val);

        2
    }

    /*
        Load value from A into constant address n16
     */
    fn ld_mn16_a(&mut self) -> u8 {
        let val = self.reg_a;

        let addr = self.mmu.rw(self.reg_pc);
        self.reg_pc += 2;

        self.mmu.wb(addr, val);

        4
    }

    /*
        Load value from A into constant address n16 between $FF00 and $FFFF
        I _think_ these addresses are used for IO
     */
    fn ldh_mn16_a(&mut self) -> u8 {
        let val = self.reg_a;

        let addr = (self.mmu.rb(self.reg_pc) as u16) + 0xFF00;
        self.reg_pc += 1;

        self.mmu.wb(addr, val);

        3
    }

    /*
        Load value from A into constant address $FF00 + C
     */
    fn ldh_mc_a(&mut self) -> u8 {
        let val = self.reg_a;

        let addr = (self.reg_c as u16) + 0xFF00;
        self.reg_pc += 1;

        self.mmu.wb(addr, val);

        2
    }

    /*
        Load value from A into address at HL and increment HL
     */
    fn ld_hli_a(&mut self) -> u8 {
        let val = self.reg_a;

        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);

        self.mmu.wb(addr, val);

        // Increment HL
        self.reg_l = self.reg_l.wrapping_add(1); // Allow overflow
        if self.reg_l == 0 {
            self.reg_h = self.reg_h.wrapping_add(1);
        }

        2
    }

    /*
        Load value from A into address at HL and decrement HL
     */
    fn ld_hld_a(&mut self) -> u8 {
        let val = self.reg_a;

        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);

        self.mmu.wb(addr, val);

        // Decrement HL
        self.reg_l = self.reg_l.wrapping_sub(1); // Allow underflow
        if self.reg_l == 255 {
            self.reg_h = self.reg_h.wrapping_sub(1);
        }

        2
    }

    /*
        Load value from address at HL into A and increment HL
     */
    fn ld_a_hli(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);

        self.reg_a = self.mmu.rb(addr);

        // Increment HL
        self.reg_l = self.reg_l.wrapping_add(1); // Allow overflow
        if self.reg_l == 0 {
            self.reg_h = self.reg_h.wrapping_add(1);
        }

        2
    }

    /*
        Load value from address at HL into A and decrement HL
     */
    fn ld_a_hld(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);

        self.reg_a = self.mmu.rb(addr);

        // Decrement HL
        self.reg_l = self.reg_l.wrapping_sub(1); // Allow underflow
        if self.reg_l == 255 {
            self.reg_h = self.reg_h.wrapping_sub(1);
        }

        2
    }

    /*
        Load constant value n16 into register SP
     */
    fn ld_sp_n16(&mut self) -> u8 {
        self.reg_sp = self.mmu.rw(self.reg_pc);
        self.reg_pc += 2;

        3
    }

    /*
        Load SP & $FF into address N16 and SP >> 8 at address N16+1
     */
    fn ld_mn16_sp(&mut self) -> u8 {
        let addr = self.mmu.rw(self.reg_pc);
        self.reg_pc += 2;

        self.mmu.ww(addr, self.reg_sp);

        5
    }

    /*
        Add the signed value e8 to SP and store in HL
     */
    fn ld_hl_spe8(&mut self) -> u8 {
        let e8 = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;

        let e8 = self.mmu.rsb(self.reg_pc);
        self.reg_pc += 1;

        let (res, carry) = self.reg_sp.overflowing_add_signed(e8 as i16);

        self.reg_h = ((res as u16) >> 8) as u8; //TODO: Replace usages of this with pattern assignment using res.to_le_bytes?
        self.reg_l = res as u8;

        self.set_flags_u16(res, carry, false);

        3
    }

    /*
        Load register HL into register SP
     */
    fn ld_sp_hl(&mut self) -> u8 {
        self.reg_h = (self.reg_sp >> 8) as u8;
        self.reg_l = self.reg_sp as u8;

        2
    }

    /*
        #####################
        Jumps and Subroutines
        #####################
     */

    /*
        Call address n16

        This pushes the address of the instruction after the CALL on the stack, such that RET can pop it later; then, it executes an implicit JP n16.
     */
    fn call_n16(&mut self) -> u8 {
        self.reg_sp -= 2; // Next stack position?

        self.mmu.ww(self.reg_sp, self.reg_pc + 2); // Address of instruction after the CALL onto stack

        self.reg_pc = self.mmu.rw(self.reg_pc); // Set the PC to n16 (JP N16)

        6
    }

    /*
        Call address n16 if condition CC is met
     */
    fn call_cc_n16(&mut self, c: CC) -> u8 {
        let mut cycles = 3;

        let should = match c {
            CC::Z => self.reg_f & FLAG_ZERO > 0,
            CC::NZ => self.reg_f & FLAG_ZERO == 0,
            CC::C => self.reg_f & FLAG_CARRY > 0,
            CC::NC => self.reg_f & FLAG_CARRY == 0,
        };

        if should {
            self.reg_sp -= 2; // Next stack position?

            self.mmu.ww(self.reg_sp, self.reg_pc + 2); // Address of instruction after the CALL onto stack

            self.reg_pc = self.mmu.rw(self.reg_pc); // Set the PC to n16 (JP N16)

            cycles = 6;
        }

        cycles
    }

    /*
        Jump to address n16
     */
    fn jp_n16(&mut self) -> u8 {
        self.reg_pc = self.mmu.rw(self.reg_pc);

        4
    }

    /*
        Jump to address n16 if condition CC is met
     */
    fn jp_cc_n16(&mut self, c: CC) -> u8 {
        let mut cycles = 3;

        let should = match c {
            CC::Z => self.reg_f & FLAG_ZERO > 0,
            CC::NZ => self.reg_f & FLAG_ZERO == 0,
            CC::C => self.reg_f & FLAG_CARRY > 0,
            CC::NC => self.reg_f & FLAG_CARRY == 0,
        };

        if should {
            self.reg_pc = self.mmu.rw(self.reg_pc);

            cycles = 4;
        }

        cycles
    }

    /*
        Jump to address from HL
     */
    fn jp_mhl(&mut self) -> u8 {
        self.reg_pc = (self.reg_h as u16) << 8 + (self.reg_l as u16);

        1
    }

    /*
        Jump to relative address n16

        This seems to me that it should be jr_e8
     */
    fn jr_n16(&mut self) -> u8 {
        let e8 = self.mmu.rsb(self.reg_pc);
        self.reg_pc += 1;

        self.reg_pc = self.reg_pc.wrapping_add_signed(e8 as i16); // TODO: Should this wrap?

        3
    }

    /*
        Jump to relative address n16 if condition cc is met
     */
    fn jr_cc_n16(&mut self, c: CC) -> u8 {
        let mut cycles = 2;

        let should = match c {
            CC::Z => self.reg_f & FLAG_ZERO > 0,
            CC::NZ => self.reg_f & FLAG_ZERO == 0,
            CC::C => self.reg_f & FLAG_CARRY > 0,
            CC::NC => self.reg_f & FLAG_CARRY == 0,
        };

        if should {
            let e8 = self.mmu.rsb(self.reg_pc);
            self.reg_pc += 1;

            self.reg_pc = self.reg_pc.wrapping_add_signed(e8 as i16); // TODO: Should this wrap?

            cycles = 3;
        }

        cycles
    }

    /*
        Return from subroutine
     */
    fn ret(&mut self) -> u8 {
        self.reg_pc = self.mmu.rw(self.reg_sp);
        self.reg_sp += 2;

        4
    }

    /*
        Return from subroutine if condition cc is met
     */
    fn ret_cc(&mut self, c: CC) -> u8 {
        let mut cycles = 2;

        let should = match c {
            CC::Z => self.reg_f & FLAG_ZERO > 0,
            CC::NZ => self.reg_f & FLAG_ZERO == 0,
            CC::C => self.reg_f & FLAG_CARRY > 0,
            CC::NC => self.reg_f & FLAG_CARRY == 0,
        };

        if should {
            self.reg_pc = self.mmu.rw(self.reg_sp);
            self.reg_sp += 2;

            cycles = 5;
        }

        cycles
    }

    /*
        Return from subroutine and enable interrupts
     */
    fn reti(&mut self) -> u8 {
        self.ime = true;

        self.reg_pc = self.mmu.rw(self.reg_sp);
        self.reg_sp += 2;

        4
    }

    /*
        Call address vec
     */
    fn rst(&mut self, addr: RST) -> u8 {
        self.reg_sp -= 2; // Next stack position?

        self.mmu.ww(self.reg_sp, self.reg_pc + 2); // Address of instruction after the CALL onto stack

        self.reg_pc = addr as u16;

        4
    }

    /*
        ################
        Stack Operations
        ################
     */

    // Some of these have ended up in arithmetic :facepalm:

    /*
        POP register AF from the stack
     */
    fn pop_af(&mut self) -> u8 {
        self.reg_f = self.mmu.rb(self.reg_sp);
        self.reg_sp += 1;

        self.reg_a = self.mmu.rb(self.reg_sp);
        self.reg_sp += 1;

        3
    }

    /*
        POP register r16 from the stack
     */
    fn pop_r16(&mut self, r: R16) -> u8 {
        match r {
            R16::BC => {
                self.reg_c = self.mmu.rb(self.reg_sp);
                self.reg_sp += 1;

                self.reg_b = self.mmu.rb(self.reg_sp);
                self.reg_sp += 1;
            }
            R16::DE => {
                self.reg_e = self.mmu.rb(self.reg_sp);
                self.reg_sp += 1;

                self.reg_d = self.mmu.rb(self.reg_sp);
                self.reg_sp += 1;
            }
            R16::HL => {
                self.reg_l = self.mmu.rb(self.reg_sp);
                self.reg_sp += 1;

                self.reg_h = self.mmu.rb(self.reg_sp);
                self.reg_sp += 1;
            }
        }

        3
    }

    /*
        PUSH register AF onto the stack
     */
    fn push_af(&mut self) -> u8 {
        self.reg_sp -= 1;
        self.mmu.wb(self.reg_sp, self.reg_a);

        self.reg_sp -= 1;
        self.mmu.wb(self.reg_sp, self.reg_f);

        4
    }

    /*
        PUSH register R16 onto the stack
     */
    fn push_r16(&mut self, r: R16) -> u8 {
        match r {
            R16::BC => {
                self.reg_sp -= 1;
                self.mmu.wb(self.reg_sp, self.reg_b);

                self.reg_sp -= 1;
                self.mmu.wb(self.reg_sp, self.reg_c);
            }
            R16::DE => {
                self.reg_sp -= 1;
                self.mmu.wb(self.reg_sp, self.reg_d);

                self.reg_sp -= 1;
                self.mmu.wb(self.reg_sp, self.reg_e);
            }
            R16::HL => {
                self.reg_sp -= 1;
                self.mmu.wb(self.reg_sp, self.reg_h);

                self.reg_sp -= 1;
                self.mmu.wb(self.reg_sp, self.reg_l);
            }
        };

        4
    }

    /*
        ########################
        Miscellaneous Operations
        ########################
     */

    /*
        Invert C flag
     */
    fn ccf(&mut self) -> u8 {
        self.reg_f ^= FLAG_CARRY;

        1
    }

    /*
        Complement A (Invert)
     */
    fn cpl(&mut self) -> u8 {
        self.reg_a != self.reg_a;

        1
    }

    /*
        Decimal Adjust Accumulator to get a correct BCD representation after an arithmetic instruction.

        TODO: I'm reasonably certian this is to do with the half-carry which imran's implementation doesn't implement.
     */
    fn daa(&mut self) -> u8 {
        self.nop()
    }

    /*
        Disable interrupts
     */
    fn di(&mut self) -> u8 {
        self.ime = false;

        1
    }

    /*
        Enable interrupts
     */
    fn ei(&mut self) -> u8 {
        self.ime = true;

        1
    }

    /*
        Enter CPU low-power consumption mode until an interrupt occurs. The exact behavior of this instruction depends on the state of the IME flag.
     */
    fn halt(&mut self) -> u8 {
        self.halt = true;

        1
    }

    /*
        No OP
     */
    fn nop(&mut self) -> u8 {
        1
    }

    /*
        Set carry flag
     */
    fn scf(&mut self) -> u8 {
        self.reg_f |= FLAG_CARRY;

        1
    }

    /*
        Enter ultra low power mode
     */
    fn stop(&mut self) -> u8 {
        self.stop = true;

        0
    }
}