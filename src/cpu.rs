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
        }
    }
}

const FLAG_ZERO: u8 = 0x80;
const FLAG_SUB: u8 = 0x40;
// const FLAG_HALF_CARRY: u8 = 0x20;
const FLAG_CARRY: u8 = 0x10;

pub struct CPU {
    // clocks
    clock_m: u32, // should be t divided by 4
    clock_t: u32,

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

        self.ime = false;
        self.halt = false;
        self.stop = false;
    }

    /*
        Execute the next CPU operation
     */
    pub fn exec(&mut self) {
        let opc = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;
        //TODO: Wrap program counter?

        let cycles = self.map_and_execute(opc) as u32;

        self.clock_m += cycles;
        self.clock_t += cycles*4;
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
    fn cp_a_mhl(&mut self) -> u8 {
        let addr = (self.reg_h as u16) << 8 + (self.reg_l as u16);
        let val = self.mmu.rb(addr);

        let (res, carry) = self.reg_a.overflowing_sub(val);

        self.set_flags_u8(res, carry, true);

        1
    }

    /*
        Subtract the constant value n8 from A and set the flags but don't store result
     */
    fn cp_a_n8(&mut self) -> u8 {
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
    fn sbc_a_r8(&mut self, r: R8) -> u8 {
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
    fn sbc_a_mhl(&mut self) -> u8 {
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
    fn sbc_a_n8(&mut self) -> u8 {
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
    fn swap_mhl(&mut self) -> u8 {
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

        let res = val.rotate_left(1);

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

        let res = val.rotate_left(1);

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
        Load value into A from address pointed to by n16
     */
    fn ld_a_mn16(&mut self) -> u8 {
        self.reg_a = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;
        
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
        Load value from  constant address n16 between $FF00 and $FFFF into A
     */
    fn ldh_a_mn16(&mut self) -> u8 {
        let addr = (self.mmu.rb(self.reg_pc) as u16) + 0xFF00;
        self.reg_pc += 1;

        self.reg_a = self.mmu.rb(addr);

        3
    }

    /*
        Load value from  constant address $FF00 + C into A
     */
    fn ldh_a_mc(&mut self) -> u8 {
        let addr = (self.reg_c as u16) + 0xFF00;
        self.reg_pc += 1;

        self.reg_a = self.mmu.rb(addr);

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
        self.reg_a = !self.reg_a;

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

    fn xx(&mut self) -> u8 {
        println!("Unexpected operation at {}, stopping.", self.reg_pc - 1);
        self.stop = true;

        0
    }

    /*
        *************
        Call Mappings
        *************
     */

    fn map_and_execute(&mut self, opc: u8) -> u8 {
        // Converted from: http://imrannazar.com/content/files/jsgb.z80.js
        // TODO: Not sure what the performance of using a match here is going to be
        match opc { // IDE seems to think this isn't exhaustive, but rust supports integer exhaustion
            0x00 => self.nop(),
            0x01 => self.ld_r16_n16(R16::BC),
            0x02 => self.ld_mr16_a(R16::BC),
            0x03 => self.inc_r16(R16::BC),
            0x04 => self.inc_r8(R8::B),
            0x05 => self.dec_r8(R8::B),
            0x06 => self.ld_r8_n8(R8::B),
            0x07 => self.rlca(),
            0x08 => self.ld_mn16_sp(), //TODO: Imran's code has LDmmSP but no impl...
            0x09 => self.add_hl_r16(R16::BC),
            0x0A => self.ld_mr16_a(R16::BC),
            0x0B => self.dec_r16(R16::BC),
            0x0C => self.inc_r8(R8::C),
            0x0D => self.dec_r8(R8::C),
            0x0E => self.ld_r8_n8(R8::C),
            0x0F => self.rrca(),

            0x10 => self.stop(), //TODO: This is "DJNZn" in Imran's code, but https://gbdev.io/pandocs/CPU_Instruction_Set.html is telling me its stop...
            0x11 => self.ld_r16_n16(R16::DE),
            0x12 => self.ld_mr16_a(R16::DE),
            0x13 => self.inc_r16(R16::DE),
            0x14 => self.inc_r8(R8::D),
            0x15 => self.dec_r8(R8::D),
            0x16 => self.ld_r8_n8(R8::D),
            0x17 => self.rla(),
            0x18 => self.jr_n16(),
            0x19 => self.add_hl_r16(R16::DE),
            0x1A => self.ld_mr16_a(R16::DE),
            0x1B => self.dec_r16(R16::DE),
            0x1C => self.inc_r8(R8::E),
            0x1D => self.dec_r8(R8::E),
            0x1E => self.ld_r8_n8(R8::E),
            0x1F => self.rra(),

            0x20 => self.jr_cc_n16(CC::NZ),
            0x21 => self.ld_r16_n16(R16::HL),
            0x22 => self.ld_hli_a(),
            0x23 => self.inc_r16(R16::HL),
            0x24 => self.inc_r8(R8::H),
            0x25 => self.dec_r8(R8::H),
            0x26 => self.ld_r8_n8(R8::H),
            0x27 => self.daa(),
            0x28 => self.jr_cc_n16(CC::Z),
            0x29 => self.add_hl_r16(R16::HL),
            0x2A => self.ld_a_hli(),
            0x2B => self.dec_r16(R16::HL),
            0x2C => self.inc_r8(R8::L),
            0x2D => self.dec_r8(R8::L),
            0x2E => self.ld_r8_n8(R8::L),
            0x2F => self.cpl(),

            0x30 => self.jr_cc_n16(CC::NC),
            0x31 => self.ld_sp_n16(),
            0x32 => self.ld_hld_a(),
            0x33 => self.inc_sp(),
            0x34 => self.inc_mhl(),
            0x35 => self.dec_mhl(),
            0x36 => self.ld_mhl_n8(),
            0x37 => self.scf(),
            0x38 => self.jp_cc_n16(CC::C),
            0x39 => self.add_hl_sp(),
            0x3A => self.ld_a_hld(),
            0x3B => self.dec_sp(),
            0x3C => self.inc_r8(R8::A),
            0x3D => self.dec_r8(R8::A),
            0x3E => self.ld_r8_n8(R8::A),
            0x3F => self.ccf(),

            0x40 => self.ld_r8_r8(R8::B, R8::B),
            0x41 => self.ld_r8_r8(R8::B, R8::C),
            0x42 => self.ld_r8_r8(R8::B, R8::D),
            0x43 => self.ld_r8_r8(R8::B, R8::E),
            0x44 => self.ld_r8_r8(R8::B, R8::H),
            0x45 => self.ld_r8_r8(R8::B, R8::L),
            0x46 => self.ld_r8_mhl(R8::B),
            0x47 => self.ld_r8_r8(R8::B, R8::A),
            0x48 => self.ld_r8_r8(R8::C, R8::B),
            0x49 => self.ld_r8_r8(R8::C, R8::C),
            0x4A => self.ld_r8_r8(R8::C, R8::D),
            0x4B => self.ld_r8_r8(R8::C, R8::E),
            0x4C => self.ld_r8_r8(R8::C, R8::H),
            0x4D => self.ld_r8_r8(R8::C, R8::L),
            0x4E => self.ld_r8_mhl(R8::C),
            0x4F => self.ld_r8_r8(R8::C, R8::A),

            0x50 => self.ld_r8_r8(R8::D, R8::B),
            0x51 => self.ld_r8_r8(R8::D, R8::C),
            0x52 => self.ld_r8_r8(R8::D, R8::D),
            0x53 => self.ld_r8_r8(R8::D, R8::E),
            0x54 => self.ld_r8_r8(R8::D, R8::H),
            0x55 => self.ld_r8_r8(R8::D, R8::L),
            0x56 => self.ld_r8_mhl(R8::D),
            0x57 => self.ld_r8_r8(R8::D, R8::A),
            0x58 => self.ld_r8_r8(R8::E, R8::B),
            0x59 => self.ld_r8_r8(R8::E, R8::C),
            0x5A => self.ld_r8_r8(R8::E, R8::D),
            0x5B => self.ld_r8_r8(R8::E, R8::E),
            0x5C => self.ld_r8_r8(R8::E, R8::H),
            0x5D => self.ld_r8_r8(R8::E, R8::L),
            0x5E => self.ld_r8_mhl(R8::E),
            0x5F => self.ld_r8_r8(R8::E, R8::A),

            0x60 => self.ld_r8_r8(R8::H, R8::B),
            0x61 => self.ld_r8_r8(R8::H, R8::C),
            0x62 => self.ld_r8_r8(R8::H, R8::D),
            0x63 => self.ld_r8_r8(R8::H, R8::E),
            0x64 => self.ld_r8_r8(R8::H, R8::H),
            0x65 => self.ld_r8_r8(R8::H, R8::L),
            0x66 => self.ld_r8_mhl(R8::H),
            0x67 => self.ld_r8_r8(R8::H, R8::A),
            0x68 => self.ld_r8_r8(R8::L, R8::B),
            0x69 => self.ld_r8_r8(R8::L, R8::C),
            0x6A => self.ld_r8_r8(R8::L, R8::D),
            0x6B => self.ld_r8_r8(R8::L, R8::E),
            0x6C => self.ld_r8_r8(R8::L, R8::H),
            0x6D => self.ld_r8_r8(R8::L, R8::L),
            0x6E => self.ld_r8_mhl(R8::L),
            0x6F => self.ld_r8_r8(R8::L, R8::A),

            0x70 => self.ld_mhl_r8(R8::B),
            0x71 => self.ld_mhl_r8(R8::C),
            0x72 => self.ld_mhl_r8(R8::D),
            0x73 => self.ld_mhl_r8(R8::E),
            0x74 => self.ld_mhl_r8(R8::H),
            0x75 => self.ld_mhl_r8(R8::L),
            0x76 => self.halt(),
            0x77 => self.ld_mhl_r8(R8::A),
            0x78 => self.ld_r8_r8(R8::A, R8::B),
            0x79 => self.ld_r8_r8(R8::A, R8::C),
            0x7A => self.ld_r8_r8(R8::A, R8::D),
            0x7B => self.ld_r8_r8(R8::A, R8::E),
            0x7C => self.ld_r8_r8(R8::A, R8::H),
            0x7D => self.ld_r8_r8(R8::A, R8::L),
            0x7E => self.ld_r8_mhl(R8::A),
            0x7F => self.ld_r8_r8(R8::A, R8::A),

            0x80 => self.add_a_r8(R8::B),
            0x81 => self.add_a_r8(R8::C),
            0x82 => self.add_a_r8(R8::D),
            0x83 => self.add_a_r8(R8::E),
            0x84 => self.add_a_r8(R8::H),
            0x85 => self.add_a_r8(R8::L),
            0x86 => self.add_a_mhl(),
            0x87 => self.add_a_r8(R8::A),
            0x88 => self.adc_a_r8(R8::B),
            0x89 => self.adc_a_r8(R8::C),
            0x8A => self.adc_a_r8(R8::D),
            0x8B => self.adc_a_r8(R8::E),
            0x8C => self.adc_a_r8(R8::H),
            0x8D => self.adc_a_r8(R8::L),
            0x8E => self.adc_a_mhl(),
            0x8F => self.adc_a_r8(R8::A),

            0x90 => self.sub_a_r8(R8::B),
            0x91 => self.sub_a_r8(R8::C),
            0x92 => self.sub_a_r8(R8::D),
            0x93 => self.sub_a_r8(R8::E),
            0x94 => self.sub_a_r8(R8::H),
            0x95 => self.sub_a_r8(R8::L),
            0x96 => self.sub_a_mhl(),
            0x97 => self.sub_a_r8(R8::A),
            0x98 => self.sbc_a_r8(R8::B),
            0x99 => self.sbc_a_r8(R8::C),
            0x9A => self.sbc_a_r8(R8::D),
            0x9B => self.sbc_a_r8(R8::E),
            0x9C => self.sbc_a_r8(R8::H),
            0x9D => self.sbc_a_r8(R8::L),
            0x9E => self.sbc_a_mhl(),
            0x9F => self.sbc_a_r8(R8::A),

            0xA0 => self.and_a_r8(R8::B),
            0xA1 => self.and_a_r8(R8::C),
            0xA2 => self.and_a_r8(R8::D),
            0xA3 => self.and_a_r8(R8::E),
            0xA4 => self.and_a_r8(R8::H),
            0xA5 => self.and_a_r8(R8::L),
            0xA6 => self.and_a_mhl(),
            0xA7 => self.and_a_r8(R8::A),
            0xA8 => self.xor_a_r8(R8::B),
            0xA9 => self.xor_a_r8(R8::C),
            0xAA => self.xor_a_r8(R8::D),
            0xAB => self.xor_a_r8(R8::E),
            0xAC => self.xor_a_r8(R8::H),
            0xAD => self.xor_a_r8(R8::L),
            0xAE => self.xor_a_mhl(),
            0xAF => self.xor_a_r8(R8::A),

            0xB0 => self.or_a_r8(R8::B),
            0xB1 => self.or_a_r8(R8::C),
            0xB2 => self.or_a_r8(R8::D),
            0xB3 => self.or_a_r8(R8::E),
            0xB4 => self.or_a_r8(R8::H),
            0xB5 => self.or_a_r8(R8::L),
            0xB6 => self.or_a_mhl(),
            0xB7 => self.or_a_r8(R8::A),
            0xB8 => self.cp_a_r8(R8::B),
            0xB9 => self.cp_a_r8(R8::B),
            0xBA => self.cp_a_r8(R8::B),
            0xBB => self.cp_a_r8(R8::B),
            0xBC => self.cp_a_r8(R8::B),
            0xBD => self.cp_a_r8(R8::B),
            0xBE => self.cp_a_mhl(),
            0xBF => self.cp_a_r8(R8::B),

            0xC0 => self.ret_cc(CC::NZ),
            0xC1 => self.pop_r16(R16::BC),
            0xC2 => self.jp_cc_n16(CC::NZ),
            0xC3 => self.jp_n16(),
            0xC4 => self.call_cc_n16(CC::NZ),
            0xC5 => self.push_r16(R16::BC),
            0xC6 => self.add_a_n8(),
            0xC7 => self.rst(RST::RST00),
            0xC8 => self.ret_cc(CC::Z),
            0xC9 => self.ret(),
            0xCA => self.jp_cc_n16(CC::Z),
            0xCB => self.map_cb_and_execute(),
            0xCC => self.call_cc_n16(CC::Z),
            0xCD => self.call_n16(),
            0xCE => self.adc_a_n8(),
            0xCF => self.rst(RST::RST08),

            0xD0 => self.ret_cc(CC::NC),
            0xD1 => self.pop_r16(R16::DE),
            0xD2 => self.jp_cc_n16(CC::NC),
            0xD3 => self.xx(),
            0xD4 => self.call_cc_n16(CC::NC),
            0xD5 => self.push_r16(R16::DE),
            0xD6 => self.sub_a_n8(),
            0xD7 => self.rst(RST::RST10),
            0xD8 => self.ret_cc(CC::C),
            0xD9 => self.reti(),
            0xDA => self.jp_cc_n16(CC::C),
            0xDB => self.xx(),
            0xDC => self.call_cc_n16(CC::C),
            0xDD => self.xx(),
            0xDE => self.sbc_a_n8(),
            0xDF => self.rst(RST::RST18),

            0xE0 => self.ldh_mn16_a(),
            0xE1 => self.pop_r16(R16::HL),
            0xE2 => self.ldh_mc_a(),
            0xE3 => self.xx(),
            0xE4 => self.xx(),
            0xE5 => self.push_r16(R16::HL),
            0xE6 => self.and_a_n8(),
            0xE7 => self.rst(RST::RST20),
            0xE8 => self.add_sp_e8(),
            0xE9 => self.jp_mhl(),
            0xEA => self.ld_mn16_a(),
            0xEB => self.xx(),
            0xEC => self.xx(),
            0xED => self.xx(),
            0xEE => self.or_a_n8(),
            0xEF => self.rst(RST::RST28),

            0xF0 => self.ldh_a_mn16(),
            0xF1 => self.pop_af(),
            0xF2 => self.ldh_a_mc(),
            0xF3 => self.di(),
            0xF4 => self.xx(),
            0xF5 => self.push_af(),
            0xF6 => self.xor_a_n8(),
            0xF7 => self.rst(RST::RST30),
            0xF8 => self.ld_hl_spe8(),
            0xF9 => self.ld_sp_hl(),
            0xFA => self.ld_a_mn16(),
            0xFB => self.ei(),
            0xFC => self.xx(),
            0xFD => self.xx(),
            0xFE => self.cp_a_n8(),
            0xFF => self.rst(RST::RST38),
        }
    }
    
    /*
        I think this is seperate because the opcode CB has a following byte with more opcodes
     */
    fn map_cb_and_execute(&mut self) -> u8 {
        /*
            BIT U3 R8: 11001011 01bbbrrr
            BIT U3 HL: 11001011 01bbb110

            SET U3 R8: 11001011 11bbbrrr
            SET U3 HL: 11001011 11bbb110

            RES U3 R8: 11001011 10bbbrrr
            RES u3 HL: 11001011 10bbb110
         */

        let opc = self.mmu.rb(self.reg_pc);
        self.reg_pc += 1;
        
        match opc {
            0x00 => self.rlc_r8(R8::B),
            0x01 => self.rlc_r8(R8::C),
            0x02 => self.rlc_r8(R8::D),
            0x03 => self.rlc_r8(R8::E),
            0x04 => self.rlc_r8(R8::H),
            0x05 => self.rlc_r8(R8::L),
            0x06 => self.rlc_mhl(),
            0x07 => self.rlc_r8(R8::A),
            0x08 => self.rrc_r8(R8::B),
            0x09 => self.rrc_r8(R8::C),
            0x0A => self.rrc_r8(R8::D),
            0x0B => self.rrc_r8(R8::E),
            0x0C => self.rrc_r8(R8::H),
            0x0D => self.rrc_r8(R8::L),
            0x0E => self.rrc_mhl(),
            0x0F => self.rrc_r8(R8::A),

            0x10 => self.rl_r8(R8::B),
            0x11 => self.rl_r8(R8::C),
            0x12 => self.rl_r8(R8::D),
            0x13 => self.rl_r8(R8::E),
            0x14 => self.rl_r8(R8::H),
            0x15 => self.rl_r8(R8::L),
            0x16 => self.rl_mhl(),
            0x17 => self.rl_r8(R8::A),
            0x18 => self.rr_r8(R8::B),
            0x19 => self.rr_r8(R8::C),
            0x1A => self.rr_r8(R8::D),
            0x1B => self.rr_r8(R8::E),
            0x1C => self.rr_r8(R8::H),
            0x1D => self.rr_r8(R8::L),
            0x1E => self.rr_mhl(),
            0x1F => self.rr_r8(R8::A),

            0x20 => self.sla_r8(R8::B),
            0x21 => self.sla_r8(R8::C),
            0x22 => self.sla_r8(R8::D),
            0x23 => self.sla_r8(R8::E),
            0x24 => self.sla_r8(R8::H),
            0x25 => self.sla_r8(R8::L),
            0x26 => self.sla_mhl(),
            0x27 => self.sla_r8(R8::A),
            0x28 => self.sra_r8(R8::B),
            0x29 => self.sra_r8(R8::C),
            0x2A => self.sra_r8(R8::D),
            0x2B => self.sra_r8(R8::E),
            0x2C => self.sra_r8(R8::H),
            0x2D => self.sra_r8(R8::L),
            0x2E => self.sra_mhl(),
            0x2F => self.sra_r8(R8::A),

            0x30 => self.swap_r8(R8::B),
            0x31 => self.swap_r8(R8::C),
            0x32 => self.swap_r8(R8::D),
            0x33 => self.swap_r8(R8::E),
            0x34 => self.swap_r8(R8::H),
            0x35 => self.swap_r8(R8::L),
            0x36 => self.swap_mhl(),
            0x37 => self.swap_r8(R8::A),
            0x38 => self.srl_r8(R8::B),
            0x39 => self.srl_r8(R8::C),
            0x3A => self.srl_r8(R8::D),
            0x3B => self.srl_r8(R8::E),
            0x3C => self.srl_r8(R8::H),
            0x3D => self.srl_r8(R8::L),
            0x3E => self.srl_mhl(),
            0x3F => self.srl_r8(R8::A),

            0x40 => self.bit_u3_r8(0, R8::B),
            0x41 => self.bit_u3_r8(0, R8::C),
            0x42 => self.bit_u3_r8(0, R8::D),
            0x43 => self.bit_u3_r8(0, R8::E),
            0x44 => self.bit_u3_r8(0, R8::H),
            0x45 => self.bit_u3_r8(0, R8::L),
            0x46 => self.bit_u3_mhl(0),
            0x47 => self.bit_u3_r8(0, R8::A),
            0x48 => self.bit_u3_r8(1, R8::B),
            0x49 => self.bit_u3_r8(1, R8::C),
            0x4A => self.bit_u3_r8(1, R8::D),
            0x4B => self.bit_u3_r8(1, R8::E),
            0x4C => self.bit_u3_r8(1, R8::H),
            0x4D => self.bit_u3_r8(1, R8::L),
            0x4E => self.bit_u3_mhl(1),
            0x4F => self.bit_u3_r8(1, R8::A),

            0x50 => self.bit_u3_r8(2, R8::B),
            0x51 => self.bit_u3_r8(2, R8::C),
            0x52 => self.bit_u3_r8(2, R8::D),
            0x53 => self.bit_u3_r8(2, R8::E),
            0x54 => self.bit_u3_r8(2, R8::H),
            0x55 => self.bit_u3_r8(2, R8::L),
            0x56 => self.bit_u3_mhl(2),
            0x57 => self.bit_u3_r8(2, R8::A),
            0x58 => self.bit_u3_r8(3, R8::B),
            0x59 => self.bit_u3_r8(3, R8::C),
            0x5A => self.bit_u3_r8(3, R8::D),
            0x5B => self.bit_u3_r8(3, R8::E),
            0x5C => self.bit_u3_r8(3, R8::H),
            0x5D => self.bit_u3_r8(3, R8::L),
            0x5E => self.bit_u3_mhl(3),
            0x5F => self.bit_u3_r8(3, R8::A),

            0x60 => self.bit_u3_r8(4, R8::B),
            0x61 => self.bit_u3_r8(4, R8::C),
            0x62 => self.bit_u3_r8(4, R8::D),
            0x63 => self.bit_u3_r8(4, R8::E),
            0x64 => self.bit_u3_r8(4, R8::H),
            0x65 => self.bit_u3_r8(4, R8::L),
            0x66 => self.bit_u3_mhl(4),
            0x67 => self.bit_u3_r8(4, R8::A),
            0x68 => self.bit_u3_r8(5, R8::B),
            0x69 => self.bit_u3_r8(5, R8::C),
            0x6A => self.bit_u3_r8(5, R8::D),
            0x6B => self.bit_u3_r8(5, R8::E),
            0x6C => self.bit_u3_r8(5, R8::H),
            0x6D => self.bit_u3_r8(5, R8::L),
            0x6E => self.bit_u3_mhl(5),
            0x6F => self.bit_u3_r8(5, R8::A),

            0x70 => self.bit_u3_r8(6, R8::B),
            0x71 => self.bit_u3_r8(6, R8::C),
            0x72 => self.bit_u3_r8(6, R8::D),
            0x73 => self.bit_u3_r8(6, R8::E),
            0x74 => self.bit_u3_r8(6, R8::H),
            0x75 => self.bit_u3_r8(6, R8::L),
            0x76 => self.bit_u3_mhl(6),
            0x77 => self.bit_u3_r8(6, R8::A),
            0x78 => self.bit_u3_r8(7, R8::B),
            0x79 => self.bit_u3_r8(7, R8::C),
            0x7A => self.bit_u3_r8(7, R8::D),
            0x7B => self.bit_u3_r8(7, R8::E),
            0x7C => self.bit_u3_r8(7, R8::H),
            0x7D => self.bit_u3_r8(7, R8::L),
            0x7E => self.bit_u3_mhl(7),
            0x7F => self.bit_u3_r8(7, R8::A),

            0x80 => self.res_u3_r8(0, R8::B),
            0x81 => self.res_u3_r8(0, R8::C),
            0x82 => self.res_u3_r8(0, R8::D),
            0x83 => self.res_u3_r8(0, R8::E),
            0x84 => self.res_u3_r8(0, R8::H),
            0x85 => self.res_u3_r8(0, R8::L),
            0x86 => self.res_u3_mhl(0),
            0x87 => self.res_u3_r8(0, R8::A),
            0x88 => self.res_u3_r8(1, R8::B),
            0x89 => self.res_u3_r8(1, R8::C),
            0x8A => self.res_u3_r8(1, R8::D),
            0x8B => self.res_u3_r8(1, R8::E),
            0x8C => self.res_u3_r8(1, R8::H),
            0x8D => self.res_u3_r8(1, R8::L),
            0x8E => self.res_u3_mhl(1),
            0x8F => self.res_u3_r8(1, R8::A),

            0x90 => self.res_u3_r8(2, R8::B),
            0x91 => self.res_u3_r8(2, R8::C),
            0x92 => self.res_u3_r8(2, R8::D),
            0x93 => self.res_u3_r8(2, R8::E),
            0x94 => self.res_u3_r8(2, R8::H),
            0x95 => self.res_u3_r8(2, R8::L),
            0x96 => self.res_u3_mhl(2),
            0x97 => self.res_u3_r8(2, R8::A),
            0x98 => self.res_u3_r8(3, R8::B),
            0x99 => self.res_u3_r8(3, R8::C),
            0x9A => self.res_u3_r8(3, R8::D),
            0x9B => self.res_u3_r8(3, R8::E),
            0x9C => self.res_u3_r8(3, R8::H),
            0x9D => self.res_u3_r8(3, R8::L),
            0x9E => self.res_u3_mhl(3),
            0x9F => self.res_u3_r8(3, R8::A),

            0xA0 => self.res_u3_r8(4, R8::B),
            0xA1 => self.res_u3_r8(4, R8::C),
            0xA2 => self.res_u3_r8(4, R8::D),
            0xA3 => self.res_u3_r8(4, R8::E),
            0xA4 => self.res_u3_r8(4, R8::H),
            0xA5 => self.res_u3_r8(4, R8::L),
            0xA6 => self.res_u3_mhl(4),
            0xA7 => self.res_u3_r8(4, R8::A),
            0xA8 => self.res_u3_r8(5, R8::B),
            0xA9 => self.res_u3_r8(5, R8::C),
            0xAA => self.res_u3_r8(5, R8::D),
            0xAB => self.res_u3_r8(5, R8::E),
            0xAC => self.res_u3_r8(5, R8::H),
            0xAD => self.res_u3_r8(5, R8::L),
            0xAE => self.res_u3_mhl(5),
            0xAF => self.res_u3_r8(5, R8::A),

            0xB0 => self.res_u3_r8(6, R8::B),
            0xB1 => self.res_u3_r8(6, R8::C),
            0xB2 => self.res_u3_r8(6, R8::D),
            0xB3 => self.res_u3_r8(6, R8::E),
            0xB4 => self.res_u3_r8(6, R8::H),
            0xB5 => self.res_u3_r8(6, R8::L),
            0xB6 => self.res_u3_mhl(6),
            0xB7 => self.res_u3_r8(6, R8::A),
            0xB8 => self.res_u3_r8(7, R8::B),
            0xB9 => self.res_u3_r8(7, R8::C),
            0xBA => self.res_u3_r8(7, R8::D),
            0xBB => self.res_u3_r8(7, R8::E),
            0xBC => self.res_u3_r8(7, R8::H),
            0xBD => self.res_u3_r8(7, R8::L),
            0xBE => self.res_u3_mhl(7),
            0xBF => self.res_u3_r8(7, R8::A),

            0xC0 => self.set_u3_r8(0, R8::B),
            0xC1 => self.set_u3_r8(0, R8::C),
            0xC2 => self.set_u3_r8(0, R8::D),
            0xC3 => self.set_u3_r8(0, R8::E),
            0xC4 => self.set_u3_r8(0, R8::H),
            0xC5 => self.set_u3_r8(0, R8::L),
            0xC6 => self.set_u3_mhl(0),
            0xC7 => self.set_u3_r8(0, R8::A),
            0xC8 => self.set_u3_r8(1, R8::B),
            0xC9 => self.set_u3_r8(1, R8::C),
            0xCA => self.set_u3_r8(1, R8::D),
            0xCB => self.set_u3_r8(1, R8::E),
            0xCC => self.set_u3_r8(1, R8::H),
            0xCD => self.set_u3_r8(1, R8::L),
            0xCE => self.set_u3_mhl(1),
            0xCF => self.set_u3_r8(1, R8::A),

            0xD0 => self.set_u3_r8(2, R8::B),
            0xD1 => self.set_u3_r8(2, R8::C),
            0xD2 => self.set_u3_r8(2, R8::D),
            0xD3 => self.set_u3_r8(2, R8::E),
            0xD4 => self.set_u3_r8(2, R8::H),
            0xD5 => self.set_u3_r8(2, R8::L),
            0xD6 => self.set_u3_mhl(2),
            0xD7 => self.set_u3_r8(2, R8::A),
            0xD8 => self.set_u3_r8(3, R8::B),
            0xD9 => self.set_u3_r8(3, R8::C),
            0xDA => self.set_u3_r8(3, R8::D),
            0xDB => self.set_u3_r8(3, R8::E),
            0xDC => self.set_u3_r8(3, R8::H),
            0xDD => self.set_u3_r8(3, R8::L),
            0xDE => self.set_u3_mhl(3),
            0xDF => self.set_u3_r8(3, R8::A),

            0xE0 => self.set_u3_r8(4, R8::B),
            0xE1 => self.set_u3_r8(4, R8::C),
            0xE2 => self.set_u3_r8(4, R8::D),
            0xE3 => self.set_u3_r8(4, R8::E),
            0xE4 => self.set_u3_r8(4, R8::H),
            0xE5 => self.set_u3_r8(4, R8::L),
            0xE6 => self.set_u3_mhl(4),
            0xE7 => self.set_u3_r8(4, R8::A),
            0xE8 => self.set_u3_r8(5, R8::B),
            0xE9 => self.set_u3_r8(5, R8::C),
            0xEA => self.set_u3_r8(5, R8::D),
            0xEB => self.set_u3_r8(5, R8::E),
            0xEC => self.set_u3_r8(5, R8::H),
            0xED => self.set_u3_r8(5, R8::L),
            0xEE => self.set_u3_mhl(5),
            0xEF => self.set_u3_r8(5, R8::A),

            0xF0 => self.set_u3_r8(6, R8::B),
            0xF1 => self.set_u3_r8(6, R8::C),
            0xF2 => self.set_u3_r8(6, R8::D),
            0xF3 => self.set_u3_r8(6, R8::E),
            0xF4 => self.set_u3_r8(6, R8::H),
            0xF5 => self.set_u3_r8(6, R8::L),
            0xF6 => self.set_u3_mhl(6),
            0xF7 => self.set_u3_r8(6, R8::A),
            0xF8 => self.set_u3_r8(7, R8::B),
            0xF9 => self.set_u3_r8(7, R8::C),
            0xFA => self.set_u3_r8(7, R8::D),
            0xFB => self.set_u3_r8(7, R8::E),
            0xFC => self.set_u3_r8(7, R8::H),
            0xFD => self.set_u3_r8(7, R8::L),
            0xFE => self.set_u3_mhl(7),
            0xFF => self.set_u3_r8(7, R8::A),
        }
    }
}