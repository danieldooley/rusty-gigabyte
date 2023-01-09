
pub struct MMU {}

pub fn new_mmu() -> MMU {
    MMU {}
}

impl MMU {

    /*
        Read byte
     */
    pub fn rb(&self, addr: u16) -> u8 {
        0
    }

    /*
        Read signed byte
     */
    pub fn rsb(&self, addr: u16) -> i8 {
        0
    }

    /*
        Read word
     */
    pub fn rw(&self, addr: u16) -> u16 {
        0
    }

    /*
        Write byte
     */
    pub fn wb(&self, addr: u16, val: u8) {

    }

    /*
        Write word
     */
    pub fn ww(&self, addr: u16, val: u16) {

    }

}