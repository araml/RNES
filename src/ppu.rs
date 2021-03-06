extern crate sdl2;

// NES
use utils::*;
use mem::{Memory as Mem};
use enums::{MemState, Interrupt};
use scroll::Scroll;

// std
use std::fmt;
use std::num::Wrapping as W;
use std::ops::{Index, IndexMut};

macro_rules! attr_bit {
    ($tile:expr, $fine_x:expr) =>
        (($tile & (0xC0000000 >> ($fine_x * 2))) >> ((15 - $fine_x) * 2))
}

macro_rules! tile_bit {
    ($tile:expr, $fine_x:expr) =>
        (($tile & (0x8000 >> $fine_x)) >> (15 - $fine_x))
}

const CTRL_SPRITE_PATTERN       : u8 = 0x08;
const CTRL_NMI                  : u8 = 0x80;

const STATUS_SPRITE_OVERFLOW    : u8 = 0x20;
const STATUS_SPRITE_0_HIT       : u8 = 0x40;
const STATUS_VBLANK             : u8 = 0x80;

const PALETTE_SIZE              : usize = 0x20;
const PALETTE_ADDRESS           : usize = 0x3f00;

// Resolution
pub const SCANLINE_WIDTH        : usize = 256;
pub const SCANLINE_COUNT        : usize = 240;

// TODO: Wait for arbitrary size array default impls to remove Scanline
pub struct Scanline(pub [u8; SCANLINE_WIDTH]);

impl Scanline {
    fn new() -> Scanline {
        Scanline([0u8; SCANLINE_WIDTH])
    }
}

impl Clone for Scanline {
    fn clone(&self) -> Scanline {
        Scanline(self.0)
    }
}

impl Index<usize> for Scanline {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        &self.0[index]
    }
}

impl IndexMut<usize> for Scanline {
    fn index_mut(&mut self, index: usize) -> &mut u8 {
        &mut self.0[index]
    }
}

#[derive(Copy, Clone, Default)]
pub struct PpuReadRegs {
    pub data    : u8,
    pub oam     : u8,
    pub status  : u8,
}

pub struct Ppu {
    palette         : [u8; PALETTE_SIZE],
    oam             : Oam,
    address         : Scroll,
    // Registers
    ctrl            : u8,
    mask            : u8,
    status          : u8,
    data_buffer     : u8,
    // Scanline should count up until the total numbers of scanlines (262)
    scanline        : usize,
    // Each scanline has 341 cycles
    scycle          : usize,
    cycles          : u32,
    sprites         : [Sprite; 0x08],
    background      : Background,
    frames          : u64,
    frame_data      : Box<[Scanline]>,
}


impl Ppu {
    pub fn new () -> Ppu {
        Ppu {
            palette         : [0; PALETTE_SIZE],
            oam             : Oam::default(),
            address         : Scroll::default(),

            ctrl            : 0,
            mask            : 0,
            status          : 0,
            data_buffer     : 0,

            scanline        : 0,
            scycle          : 0,
            cycles          : 0,

            sprites         : [Sprite::default(); 8],
            background      : Background::default(),

            frames          : 0,
            frame_data      : vec![Scanline::new(); SCANLINE_COUNT]
                                  .into_boxed_slice(),
        }
    }

    pub fn cycle(&mut self, memory: &mut Mem) {
        // Update PPU with what the CPU hay have sent to memory latch
        self.ls_latches(memory);
        if self.render_on() {
            match (self.scycle, self.scanline) {
                // Idle scanlines
                (_, 240...260) => (),
                // Last scanline, updates vertical scroll
                (280...304, 261) => self.address.copy_vertical(),
                // Dot 257 updates horizontal scroll
                (257, _) => self.address.copy_horizontal(),
                // Overlaps with above but nothing really happens in 257
                (257...320, _) => {
                    // This syncs with sprite evaluation in oam
                    self.fetch_sprite(memory);
                }
                // At dot 1 of prerender we need to unset the sprite bits
                (1, 261) => self.status &= !(STATUS_SPRITE_0_HIT |
                                             STATUS_SPRITE_OVERFLOW),
                // Idle cycles
                (0, _) | (337...340, _) => (),
                _ => {
                    // These are fetching scanlines, including prerender
                    // (1...256 + 321..336, 0...239 + 261)
                    if self.scycle < 257 && self.scanline != 261 {
                        self.draw_dot();
                        // Decrement sprite counters or shift their tile data
                        for s in self.sprites.iter_mut() {
                            s.decrement_or_shift();
                        }
                    }
                    self.background.shift();
                    self.background.fetch(memory, &self.address, self.scycle);
                    if self.scycle % 8 == 0 {
                        // Increment horizontal scroll after each full fetch
                        self.address.increment_coarse_x();
                        if self.scycle == 256 {
                            self.address.increment_y();
                        }
                    }
                }
            }
            // OAM works at rendering lines
            let big_sprites = self.sprite_big();
            if self.scanline < 240 &&
               self.oam.cycle(self.scycle, self.scanline as u8,
                              &mut self.sprites, big_sprites) {
                set_flag!(self.status, STATUS_SPRITE_OVERFLOW);
            }
        }
        // VBLANK
        if self.scycle == 1 && self.scanline == 241 {
            set_flag!(self.status, STATUS_VBLANK);
            if is_flag_set!(self.ctrl, CTRL_NMI) {
                memory.set_interrupt(Interrupt::NMI);
            }
        } else if self.scycle == 1 && self.scanline == 261 {
            unset_flag!(self.status, STATUS_VBLANK);
        }
        // TODO
        // When render is not activated the loop is shorter
        // if !self.render_on() && self.cycles == VBLANK_END_NO_RENDER {}

        // Reset values at the end of scanlines
        if self.scycle == 340 && self.scanline == 261 {
            // TODO: Skip a cycle on odd frames and background on
            self.scycle = 0;
            self.scanline = 0;
            self.cycles = 0;
            self.frames += 1;
        } else if self.scycle == 340 {
            // If we finished the current scanline we pass to the next one
            self.scanline += 1;
            self.scycle = 0;
        } else {
            self.scycle += 1;
            self.cycles += 1;
        }
        let read_regs = PpuReadRegs {
                data    : self.data_buffer,
                oam     : self.oam.load_data(),
                status  : self.status,
        };
        // Update memory PPU registers copy
        memory.set_ppu_read_regs(read_regs);
    }

    fn fetch_sprite(&mut self, memory: &mut Mem) {
        let big_sprites = self.sprite_big();
        let table = self.sprite_table();
        let sprite = &mut self.sprites[((self.scycle - 1) / 8) % 8];
        // Get fine Y position
        let mut y_offset = W16!(W(self.scanline as u8) - sprite.y_pos);
        if sprite.flip_vertically() {
            y_offset = W(if big_sprites {15} else {7}) - y_offset;
        }
        // With big sprites we need to jump to the next tile
        if y_offset >= W(8) {
            y_offset += W(8)
        }
        // Compose the table and the tile address with the fine Y position
        let address = if big_sprites {
            (W16!(W(sprite.tile.0.rotate_right(1))) << 5) | y_offset
        } else {
            table | (W16!(sprite.tile) << 4) | y_offset
        };
        match (self.scycle - 1) % 8 {
            3 => sprite.latch = sprite.attributes.0,
            4 => sprite.counter = sprite.x_pos.0,
            5 => {
                sprite.lshift = memory.chr_load(address).0;
                if !sprite.flip_horizontally() {
                    sprite.lshift = reverse_byte(sprite.lshift);
                }
            },
            7 => {
                sprite.hshift = memory.chr_load(address + W(8)).0;
                if !sprite.flip_horizontally() {
                    sprite.hshift = reverse_byte(sprite.hshift);
                }
            },
            _ => {},
        }
    }

    fn draw_dot(&mut self) {
        let mut back_index = 0;
        if self.show_background() {
            let fine_x = self.address.get_fine_x();
            back_index = self.background.get_palette_index(fine_x);
        }
        // Assume we are going to draw the background or the back color
        let mut color_index = self.palette[back_index];
        if self.show_sprites() {
            // Amount of sprites in this scanline
            let cnt = self.oam.count();
            // Look for the first sprite that has a pixel to draw
            let index = self.sprites[..cnt].iter().position(Sprite::has_pixel);
            if let Some(index) = index {
                let sprite = &self.sprites[index];
                let sprite_front = sprite.get_priority();
                // We have a sprite pixel, we should check for sprite 0 hit
                if self.oam.sprite_zero_hit() && index == 0 &&
                   back_index != 0 && self.scycle != 256 {
                    self.status |= STATUS_SPRITE_0_HIT;
                }
                if sprite_front || back_index == 0 {
                    color_index = self.palette[sprite.get_palette_index()];
                }
            }
        }
        self.frame_data[self.scanline][self.scycle - 1] = color_index;
    }

    fn sprite_big(&self) -> bool {
        is_flag_set!(self.ctrl, 0x20)
    }

    fn sprite_table(&self) -> W<u16> {
        if is_flag_set!(self.ctrl, CTRL_SPRITE_PATTERN) {
            W(0x1000)
        } else {
            W(0)
        }
    }

    fn render_on(&self) -> bool {
        self.show_sprites() || self.show_background()
    }

    fn rendering(&self) -> bool {
        self.render_on() && (self.scanline < 240 || self.scanline == 261)
    }

    fn show_sprites(&self) -> bool {
        is_flag_set!(self.mask, 0x10)
    }

    fn show_background(&self) -> bool {
        is_flag_set!(self.mask, 0x08)
    }

    /* load store latches */
    fn ls_latches(&mut self, memory: &mut Mem) {
        let (latch, status) = memory.get_latch();
        match status {
            MemState::PpuCtrl   => {
                if !is_flag_set!(self.ctrl, CTRL_NMI) &&
                    is_flag_set!(latch.0, CTRL_NMI) &&
                    is_flag_set!(self.status, STATUS_VBLANK) {
                    memory.set_interrupt(Interrupt::NMI);
                }
                self.ctrl = latch.0;
                self.address.set_ppuctrl(latch);
            },
            MemState::PpuMask   => { self.mask = latch.0; },
            MemState::OamAddr   => { self.oam.set_address(latch); },
            MemState::OamData   => { self.oam.store_data(latch); },
            MemState::PpuScroll => { self.address.set_scroll(latch); },
            MemState::PpuAddr   => { self.address.set_address(latch); },
            MemState::PpuData   => { self.store(memory, latch);},
            _                   => (),
        }

        let read_status = memory.ppu_load_status();

        match read_status {
            MemState::PpuStatus => {
                self.address.reset();
                unset_flag!(self.status, STATUS_VBLANK);
            },
            MemState::PpuData => {
                self.data_buffer = self.load(memory).0;
            }
            _                   => {},
        }
    }

    fn palette_mirror(&mut self, address: usize) -> usize {
        let index = address & (PALETTE_SIZE - 1);
        // Mirroring 0x10/0x14/0x18/0x1C to lower address
        if (index & 0x3) == 0 {
            index & 0xF
        } else {
            index
        }
    }

    fn load(&mut self, memory: &mut Mem) -> W<u8> {
        let rendering = self.rendering();
        let address = self.address.get_address(rendering);
        let addr = address.0 as usize;
        if addr < PALETTE_ADDRESS {
            memory.chr_load(address)
        } else {
            W(self.palette[self.palette_mirror(addr)])
        }
    }

    fn store(&mut self, memory: &mut Mem, value: W<u8>) {
        let rendering = self.rendering();
        let address = self.address.get_address(rendering);
        let addr = address.0 as usize;
        if addr < PALETTE_ADDRESS {
            memory.chr_store(address, value);
        } else {
            self.palette[self.palette_mirror(addr)] = value.0 & 0x3F;
        }
    }

    pub fn frame_data(&self) -> (u64, &[Scanline]) {
        (self.frames, &self.frame_data)
    }
}

impl Default for Ppu {
    fn default () -> Ppu {
        Ppu::new()
    }
}


impl fmt::Debug for Ppu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PPU: \n OAM: {:?}, ctrl: {:?}, mask: {:?}, status: {:?}, \
                   address: {:?}",
               self.oam, self.ctrl, self.mask, self.status, self.address)
    }
}

#[derive(Copy, Clone, Default)]
struct Background {
    ltile_shift     : u16,
    htile_shift     : u16,
    attr_shift      : u32,
    next_ltile      : W<u8>,
    next_htile      : W<u8>,
    next_attr       : W<u8>,
    next_name       : W<u8>,
}

impl Background {

    fn fetch(&mut self, memory: &mut Mem, scroll: &Scroll, scycle: usize) {
        // First cycle is idle
        match (scycle - 1) & 0x7 {
            // if on a visible scanline
            1 => {
                let address = scroll.get_nametable_address();
                self.next_name = memory.chr_load(address);
            },
            3 => {
                let address = scroll.get_attribute_address();
                self.next_attr = memory.chr_load(address);
            },
            5 => {
                let index = self.next_name;
                let address = scroll.get_tile_address(index);
                self.next_ltile = memory.chr_load(address);
            },
            7 => {
                let index = self.next_name;
                let address = scroll.get_tile_address(index);
                self.next_htile = memory.chr_load(address + W(8));
                self.set_shift_regs(scroll);
            },
            _ => {},
        }
    }

    fn shift(&mut self) {
        self.ltile_shift <<= 1;
        self.htile_shift <<= 1;
        self.attr_shift <<= 2;
    }

    fn set_shift_regs(&mut self, scroll: &Scroll) {
        self.ltile_shift = self.ltile_shift & 0xFF00 | self.next_ltile.0 as u16;
        self.htile_shift = self.htile_shift & 0xFF00 | self.next_htile.0 as u16;
        let attr = scroll.get_tile_attribute(self.next_attr).0 as u32;
        // attr is a 2 bit palette index, broadcast that into 16 bits
        self.attr_shift = self.attr_shift & 0xFFFF0000 | (attr * 0x5555);
    }

    fn get_color_index(&self, fine_x: u8) -> usize {
        (tile_bit!(self.ltile_shift, fine_x) |
         tile_bit!(self.htile_shift, fine_x) << 1) as usize
    }

    fn get_palette_index(&self, fine_x: u8) -> usize {
        let back_index = self.get_color_index(fine_x);
        if back_index > 0 {
            (attr_bit!(self.attr_shift, fine_x) as usize) * 4 + back_index
        } else {
            0
        }
    }
}

#[derive(Copy, Clone, Default)]
struct Sprite {
    pub y_pos       : W<u8>,
    pub tile        : W<u8>,
    pub attributes  : W<u8>,
    pub x_pos       : W<u8>,
    pub counter     : u8,
    pub latch       : u8,
    pub lshift      : u8,
    pub hshift      : u8,
}

impl Sprite {

    pub fn set_sprite_info(&mut self, index: usize, value: W<u8>) {
        match index {
            0 => self.y_pos = value,
            1 => self.tile = value,
            2 => self.attributes = value,
            3 => self.x_pos = value,
            _ => unreachable!(),
        }
    }

    pub fn get_palette_index(&self) -> usize {
        let sprite_index = (self.lshift & 1) | ((self.hshift & 1) << 1);
        (self.get_palette() + 4) * 4 + sprite_index as usize
    }

    pub fn decrement_or_shift(&mut self) {
        if self.counter > 0 {
            self.counter -= 1;
        } else {
            self.lshift >>= 1;
            self.hshift >>= 1;
        }
    }

    pub fn get_priority(&self) -> bool {
        !is_flag_set!(self.attributes.0, 0x20)
    }

    pub fn get_palette(&self) -> usize {
        (self.attributes.0 & 3) as usize
    }

    pub fn flip_horizontally(&self) -> bool {
        is_flag_set!(self.attributes.0, 0x40)
    }

    pub fn flip_vertically(&self) -> bool {
        is_flag_set!(self.attributes.0, 0x80)
    }

    pub fn has_pixel(&self) -> bool {
        self.counter == 0 && (self.lshift & 1 != 0 || self.hshift & 1 != 0)
    }
}

struct Oam {
    mem             : [u8; 0x100],
    smem            : [u8; 0x20],
    mem_index       : W<u8>,
    smem_index      : usize,
    address         : W<u8>,
    count           : usize,
    read            : u8,
    next_sprite     : bool,
    zero_hit_now    : bool,
    zero_hit_next   : bool,
}

impl Default for Oam {
    fn default() -> Oam {
        Oam {
            mem             : [0; 0x100],
            smem            : [0; 0x20],
            mem_index       : W(0),
            smem_index      : 0,
            address         : W(0),
            count           : 0,
            read            : 0,
            next_sprite     : false,
            zero_hit_now    : false,
            zero_hit_next   : false,
        }
    }
}

impl fmt::Debug for Oam {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut output = "OAM: mem: \n".to_string();
        print_mem(&mut output, &self.mem[..]);
        write!(f, "{}", output)
    }
}

impl Oam {

    fn load_data(&mut self) -> u8 {
        self.mem[self.address.0 as usize]
    }

    fn store_data(&mut self, value: W<u8>) {
        self.mem[self.address.0 as usize] = value.0;
        self.address += W(1);
    }

    fn set_address(&mut self, addr: W<u8>) {
        self.address = addr;
    }

    // The amount of sprites we found
    fn count(&self) -> usize {
        self.count
    }

    // If Sprite Zero did hit
    fn sprite_zero_hit(&self) -> bool {
        self.zero_hit_now
    }

    pub fn cycle(&mut self, cycles: usize, scanline: u8,
                 spr_units: &mut [Sprite], big_sprites: bool) -> bool {
        if cycles == 0 {
            // Sprite zero hit for the current scanline was in the previous one
            self.zero_hit_now = self.zero_hit_next;
            self.zero_hit_next = false;
            self.mem_index = W(0);
            self.smem_index = 0;
            return false;
        }
        let cycles = cycles - 1;
        if cycles < 64 {
            // Fill OAM
            if cycles % 2 == 0 {
                self.read = 0xFF;
            } else {
                self.smem[cycles >> 1] = self.read;
            }
        } else if cycles < 256 {
            // Read on even cycles
            if cycles % 2 == 0 {
                self.read = self.mem[self.mem_index.0 as usize];
                return false;
            }
            if self.smem_index % 4 != 0 {
                // Copy the rest of the sprite data when previous was in range
                self.smem[self.smem_index] = self.read;
                self.smem_index += 1;
                self.mem_index += W(1);
            } else if self.smem_index < 0x20 {
                // Copy the Y coordinate and test if in range
                self.smem[self.smem_index] = self.read;
                // If sprite is in range copy the rest, else go to the next one
                if self.in_range(scanline, big_sprites) {
                    // Sprite Zero Hit
                    if self.mem_index == W(0) {
                        self.zero_hit_next = true;
                    }
                    self.smem_index += 1;
                    self.mem_index += W(1);
                } else {
                    self.mem_index += W(4);
                }
            } else {
                // 8 sprite limit reached, look for sprite overflow
                if self.mem_index.0 % 4 != 0 && !self.next_sprite {
                    self.mem_index += W(1);
                } else if self.in_range(scanline, big_sprites) {
                    self.mem_index += W(1);
                    self.next_sprite = false;
                    // Sprite overflow
                    return true;
                } else {
                    // Emulate hardware bug, add 5 instead of 4
                    // FIXME: I think there shouldn't be a carry from bit 1 to 2
                    self.mem_index += W(5);
                    self.next_sprite = true;
                }
            }
        } else if cycles < 320 {
            // Set index to 0 at start so we can copy to the sprite units.
            // Set also the count to the amount of sprites we have found
            if cycles == 256 {
                self.count = self.smem_index / 4;
                self.smem_index = 0;
            }
            // Fill up to eight sprite units with data
            if cycles & 4 == 0 && self.smem_index < self.count * 4 {
                let data = W(self.smem[self.smem_index]);
                let sprite = &mut spr_units[self.smem_index / 4];
                sprite.set_sprite_info(self.smem_index % 4, data);
                self.smem_index += 1;
            }
        }
        return false;
    }

    fn in_range(&self, scanline: u8, big_sprites: bool) -> bool {
        let size = if big_sprites {16} else {8};
        self.read < 0xF0 && self.read + size > scanline && self.read <= scanline
    }
}
