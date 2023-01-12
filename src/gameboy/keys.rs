use std::sync::RwLock;

#[derive(Debug)]
pub enum Keys {
    A,
    B,
    START,
    SELECT,
    UP,
    DOWN,
    LEFT,
    RIGHT,
}

pub struct KeyReg {
    column: RwLock<u8>,

    rows: RwLock<[u8; 2]>,
}

pub fn new_key_reg() -> KeyReg {
    KeyReg {
        column: RwLock::new(0),

        rows: RwLock::new([0x0F, 0x0F]),
    }
}

impl KeyReg {
    pub fn set_column(&self, val: u8) {
        let mut column = self.column.write().unwrap();

        *column = val & 0x30;
    }

    pub fn get_keys(&self) -> u8 {
        let column = self.column.read().unwrap();
        let rows = self.rows.read().unwrap();

        // TODO: Is there a risk of a deadlock here?

        match *column {
            0x10 => rows[0],
            0x20 => rows[1],
            _ => 0,
        }
    }

    pub fn key_down(&self, key: Keys) {
        let mut rows = self.rows.write().unwrap();

        match key {
            Keys::A => rows[0] &= 0xE,
            Keys::B => rows[0] &= 0xD,
            Keys::START => rows[0] &= 0x7,
            Keys::SELECT => rows[0] &= 0xB,
            Keys::UP => rows[1] &= 0xB,
            Keys::DOWN => rows[1] &= 0x7,
            Keys::LEFT => rows[1] &= 0xD,
            Keys::RIGHT => rows[1] &= 0xE,
        }
    }

    pub fn key_up(&self, key: Keys) {
        let mut rows = self.rows.write().unwrap();

        match key {
            Keys::A => rows[0] |= 0x1,
            Keys::B => rows[0] |= 0x2,
            Keys::START => rows[0] |= 0x8,
            Keys::SELECT => rows[0] |= 0x4,
            Keys::UP => rows[1] |= 0x4,
            Keys::DOWN => rows[1] |= 0x8,
            Keys::LEFT => rows[1] |= 0x2,
            Keys::RIGHT => rows[1] |= 0x1,
        }
    }
}