use crate::gameboy::gpu::Mode::ScOam;
use crate::gameboy::mmu::MMU;

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

    //
    line: usize,
}

pub fn new_gpu() -> GPU {
    GPU {
        mode: Mode::HBlank,
        mode_clock: 0,
        line: 0,
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
                    } else {
                        self.mode = ScOam;
                    }
                }
            }
            Mode::VBlank => {
                if self.mode_clock >= 456 {
                    self.mode_clock = 0;
                    self.line += 1;

                    if self.line > 153 {
                        // Restart scanning modes
                        self.mode = ScOam;
                        self.line = 0;
                    }
                }
            }
        }
    }

    /*
        Writes a line to the framebuffer
     */
    fn renderscan(&mut self, mmu: &mut MMU) {
        // TODO:
    }
}