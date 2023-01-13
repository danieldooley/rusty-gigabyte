use std::{fs, io};

pub struct Cartridge {
    file: Vec<u8>,
}

pub fn new_cartridge_from_file(path: &str) -> Result<Cartridge, io::Error> {
    let file = fs::read(path)?;

    Ok(Cartridge { file })
}

pub fn new_cartridge_from_url(path: &str) -> Result<Cartridge, reqwest::Error> {
    let file = Vec::<u8>::from(reqwest::blocking::get(path)?.bytes()?);

    Ok(Cartridge { file })
}


impl Cartridge {
    pub fn read_bank_0(&self) -> [u8; 16384] {
        /*
            This will result in a clone, which probably isn't ideal for performance.
            However returning a &[u8; x] requires setting lifetimes...
         */
        self.file[0..16384].try_into().expect("incorrect bank 0 slice length")
    }

    pub fn read_bank_n(&self) -> [u8; 16384] {
        // TODO: Not sure how MBC will be handled, but im expecting to handle it within Cartridge
        self.file[16384..16384 * 2].try_into().expect("incorrect bank n slice length")
    }
}