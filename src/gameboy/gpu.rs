use std::sync::mpsc::Sender;
use std::thread::sleep;
use std::time::Duration;
use crate::gameboy::mmu::MMU;

const REG_LCD_GPU_CONTROL: u16 = 0xFF40;
const REG_SCROLL_Y: u16 = 0xFF42;
const REG_SCROLL_X: u16 = 0xFF43;
const REG_CURR_SCAN_LINE: u16 = 0xFF44;
const REG_BG_PALETTE: u16 = 0xFF47;

const FLAG_CONT_BG_ON: u8 = 0x01;
const FLAG_CONT_SPR_ON: u8 = 0x02;
const FLAG_CONT_SPR_SZ: u8 = 0x04;
// 8x8 when unset, 16x16 when set
const FLAG_CONT_BG_MAP: u8 = 0x08;
// #0 when off #1 when on (which map is in use)
const FLAG_CONT_BG_SET: u8 = 0x10;
// #0 when off #1 when on (which tileset is in use)
const FLAG_CONT_WIN_ON: u8 = 0x20;
const FLAG_CONT_WIN_TM: u8 = 0x40;
// #0 when off #1 when on (which window tilemap)
const FLAG_CONT_DISP_ON: u8 = 0x80;

const COLORS: [[u8; 3]; 4] = [
    [255, 255, 255], // OFF
    [192, 192, 192], // 33%
    [96, 96, 96], // 66%
    [0, 0, 0], // ON
];

enum Mode {
    HBlank,
    // Horizonal Blank
    VBlank,
    // Vertical Blank
    ScOam,
    // Scanline accessing OAM
    ScVram, // Scanline accessing VRAM
}

pub struct GPU {
    // Following: http://imrannazar.com/GameBoy-Emulation-in-JavaScript:-GPU-Timings
    /*
        As I understand this code is to simulate the CRT-like behaviour that the GB GPU uses
        to draw to the screen.
     */
    mode: Mode,

    // I think this is tracking how long the GPU has been in a mode, based on CPU t cycles
    mode_clock: u32,

    // Which line is currently being scanned?
    line: u8,

    // The framebuffer
    fb: Vec<u8>, // [u8; 160 * 144 * 3], // 3 bytes per pixel (RGB), 160x144 pixels.

    // The channel to dispatch the framebuffer on
    sender: Sender<Vec<u8>>
}

pub fn new_gpu(sender: Sender<Vec<u8>>) -> GPU {
    GPU {
        mode: Mode::HBlank,
        mode_clock: 0,
        line: 0,
        fb: vec![0; 69120], //[0; 69120],
        sender,
    }
}


impl GPU {
    /*
        This probably seems a bit odd. Followin Imran's guide, this is setting
        up the timings to roughly emulate the behaviour of the GPU in the GB.

        It behaves somewhat like a CRT monitor. The loop goes like:
            - Mode::ScOam & Mode::ScVram - Scanning across the screen (scanline),
              this results in the pixels being written. (Here to a framebuffer rather than to a screen)
            - Mode::HBlank - This is the time that the CRT would be returning to the beginning of the
              next line. Once we HBlank the last line of the screen we dispatch the frame.
            - Mode::VBlank - This is the time that the CRT would be returning to the start of the first
              line. This step resets the loop to the beginning.

        I would assume that because delta_t is not going to be exact, that timings may vary from
        frame to frame. TODO: This might be one place that cycle-accurate emulation may differ?
     */
    pub(crate) fn step(&mut self, mmu: &mut MMU, delta_t: u32) {
        self.mode_clock += delta_t;

        match self.mode {
            // OAM Read mode, scanline active
            Mode::ScOam => {
                if self.mode_clock >= 80 {
                    // Enter scanline mode 3
                    self.mode = Mode::ScVram;
                    self.mode_clock = 0;
                }
            }
            Mode::ScVram => {
                if self.mode_clock >= 172 {
                    // Enter HBlank
                    self.mode = Mode::HBlank;
                    self.mode_clock = 0;

                    self.renderscan(mmu);
                }
            }
            Mode::HBlank => {
                if self.mode_clock >= 204 {
                    // After the last hblank push the screen data to the window
                    self.mode_clock = 0;
                    self.line += 1;

                    if self.line == 143 {
                        self.mode = Mode::VBlank;
                        self.sender.send(self.fb.clone()).unwrap(); //TODO: Handle error?
                    } else {
                        self.mode = Mode::ScOam;
                    }
                }
            }
            Mode::VBlank => {
                if self.mode_clock >= 456 {
                    self.mode_clock = 0;
                    self.line += 1;

                    if self.line > 153 {
                        // Restart scanning modes
                        self.mode = Mode::ScOam;
                        self.line = 0;
                    }
                }
            }
        }
    }

    fn paletteify(&mut self, color: u8) -> [u8; 3] {
        COLORS[color as usize]
        //TODO: Palette
    }

    fn tilerow_n_to_color(&self, b1: u8, b2: u8, n: u8) -> u8 {
       ((b1 & (1 << n)) >> n) & (((b2 & (1 << n)) >> n) << 1) //TODO: This is a bit gross...
    }

    /*
        Writes a line to the framebuffer
     */
    fn renderscan(&mut self, mmu: &mut MMU) {
        // From: http://imrannazar.com/GameBoy-Emulation-in-JavaScript:-Graphics

        // Store the control flag value for reuse
        let control_flags = mmu.rb(REG_LCD_GPU_CONTROL);

        // VRAM offsets for the tilemap
        let mut map_offs = if control_flags & FLAG_CONT_BG_MAP == 0 { 0x9800 } else { 0x9C00 };

        let mut tile_data_offs = if control_flags & FLAG_CONT_BG_SET == 0 { 0x8800 } else { 0x8000 };

        // Get the scroll values
        let sc_y = mmu.rb(REG_SCROLL_Y);
        let sc_x = mmu.rb(REG_SCROLL_X);

        // Which line of tiles to use in the map
        map_offs += (((self.line + sc_y) & 255) >> 3) as u16; // TODO: Understand

        // Which tile to start with in the map line
        let mut line_offs = (sc_x >> 3) as u16;

        // Which line of pixels to use in the tiles
        let y = (self.line + sc_y) & 7;

        // Where in the tileline to start
        let mut x = sc_x & 7; // Get the specific pixel of the tile to grab

        // Where to render on the framebuffer
        let fb_offs = ((self.line as u32) * 160 * 3) as usize;

        // Read tile index from the background map
        let mut tile = mmu.rb(map_offs + line_offs);

        // If the tile data set in use is #1 the indices are signed: calculate a real tile offset
        // if control_flags & FLAG_CONT_BG_SET > 0 && tile < 128 {
        //     tile += 256;
        // }

        for i in 0..160 {
            let b1 = mmu.rb(tile_data_offs + (tile as u16));
            let b2 = mmu.rb(tile_data_offs + (tile as u16) + 1);

            // Re-map the tile pixel through the palette
            let color = self.paletteify(self.tilerow_n_to_color(b1, b2, x));

            // Plot the pixel to the framebuffer
            self.fb[fb_offs + (i * 3) + 0] = color[0];
            self.fb[fb_offs + (i * 3) + 1] = color[1];
            self.fb[fb_offs + (i * 3) + 2] = color[2];

            x += 1;
            if x == 8 {
                x = 0;
                line_offs = (line_offs + 1 & 31);
                tile = mmu.rb(map_offs + line_offs);
                // if control_flags & FLAG_CONT_BG_SET > 0 && tile < 128 {
                //     tile += 256;
                // }
            }
        }
    }
}