use std::num::Wrapping as W;
use std::fmt;

const PAGE_MASK         : W<u16> = W(0xFF00 as u16);

pub enum MemState {
    Ppuctrl,
    Ppumask,
    Ppustatus,
    Oamaddr,
    Oamdata,
    Ppuscroll,
    Ppuaddr,
    Ppudata,
    Oamdma,
    Io,
    Memory,
    NoState,
}

impl fmt::Display for MemState{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}",
            match *self {
                MemState::Ppuctrl   => "Ppuctrl",
                MemState::Ppumask   => "Ppumask",
                MemState::Ppustatus => "Ppustatus",
                MemState::Oamaddr   => "Oamaddr",
                MemState::Oamdata   => "Oamdata",
                MemState::Ppuscroll => "Ppuscroll",
                MemState::Ppuaddr   => "Ppuaddr",
                MemState::Ppudata   => "Ppudata",
                MemState::Oamdma    => "Oamdma",
                MemState::Memory    => "Memory",
                MemState::Io        => "Io",
                MemState::NoState   => "NoState",
        })
    }
}

pub struct Memory {
    ram : [u8; 2048],

    pub read_status     : MemState,
    pub write_status    : MemState,

    // Some registers may be removed later.
    pub ppuctrl         : u8,
    pub ppumask         : u8,
    pub ppustatus       : u8,
    pub oamaddr         : u8,
    pub oamdata         : u8,
    pub ppuscroll       : u8,
    pub ppuaddr         : u8,
    pub ppudata         : u8,
    pub oamdma          : u8,

    pub dma             : bool,

}

impl Memory {
    pub fn new () -> Memory {
        Memory {
            ram : [0;  2048],
            read_status     : MemState::NoState,
            write_status    : MemState::NoState,            

            // Some registers may be removed later.
            ppuctrl         : 0,
            ppumask         : 0,
            ppustatus       : 0,
            oamaddr         : 0,
            oamdata         : 0,
            ppuscroll       : 0,
            ppuaddr         : 0,
            ppudata         : 0,
            oamdma          : 0,

            dma             : false,
        }
    }

    pub fn load (&mut self, address: W<u16>) -> W<u8> {
        let address = address.0; 
        W(if address < 0x2000 {
            self.read_status = MemState::Memory;
            self.ram[(address & 0x7ff) as usize]
        } else if address < 0x4000 {
           match (address % 0x2000) & 0x7 {
                // En teoria los registros comentados son read only
                // 0 => self.ppuctrl
                // 1 => self.ppumask,
                2 =>    {   self.read_status = MemState::Ppustatus; 
                            self.ppustatus
                        },
                // 3 => self.oamaddr,
                4 =>    {   self.read_status = MemState::Oamdata;
                            self.oamdata
                        },
                // 5 => self.ppuscroll,
                // 6 => self.ppuaddr,
                7 =>    {   self.read_status = MemState::Ppudata;
                            self.ppudata
                        },
                _ => 0 // fuck you.
            }
        } else if address < 0x4020 {
            /* Apu AND IO TODO*/
            self.read_status = MemState::Io;
            match address {
                0x4000 => 0,
                0x4001 => 0,
                0x4002 => 0,
                0x4003 => 0,
                0x4004 => 0,
                0x4005 => 0,
                0x4006 => 0,
                0x4007 => 0,
                0x4008 => 0,
                0x4009 => 0,
                0x400A => 0,
                0x400B => 0,
                0x400C => 0,
                0x400D => 0,
                0x400E => 0,
                0x400F => 0,
                0x4010 => 0,
                0x4011 => 0,
                0x4012 => 0,
                0x4013 => 0,
                0x4014 =>   {   self.read_status = MemState::Oamdma;
                                self.oamdma
                            },
                0x4015 => 0,
                0x4016 => 0,
                0x4017 => 0,
                0x4018 => 0,
                0x4019 => 0,
                0x401A => 0,
                0x401B => 0,
                0x401C => 0,
                0x401D => 0,
                0x401E => 0, 
                0x401F => 0,
                _      => 0,
            }
        } else if address < 0x6000 {
            /* Cartridge expansion ROM the f */
            self.read_status = MemState::Memory;
            0
        } else if address < 0x8000 {
            /* SRAM */
            self.read_status = MemState::Memory;
            0
        } else /* 0x8000 <= addr < 0xC000*/ {
            /* PRG-ROM */
            self.read_status = MemState::Memory;
            0
        })
    }

    pub fn store (&mut self, address: W<u16>, value: W<u8>){
        let address = address.0; 
        let val = value.0;
        if address < 0x2000 {
            self.write_status = MemState::Memory;
            self.ram[(address & 0x7ff) as usize] = val
        } else if address < 0x4000 {
            match (address % 0x2000) & 0x7 {
                0 =>    {   self.write_status = MemState::Ppuctrl;
                            self.ppuctrl = val
                        },
                1 =>    {   self.write_status = MemState::Ppumask;
                            self.ppumask = val 
                        },
                // 2 => self.ppustatus = value, Este registro es read only
                3 =>    {   self.write_status = MemState::Oamaddr;
                            self.oamaddr = val
                        },
                4 =>    {   self.write_status = MemState::Oamdata;
                            self.oamdata = val
                        },
                5 =>    {   self.write_status = MemState::Ppuscroll;
                            self.ppuscroll = val
                        },
                6 =>    {   self.write_status = MemState::Ppuaddr;
                            self.ppuaddr = val
                        },
                7 =>    {   self.write_status = MemState::Ppudata;
                            self.ppudata = val
                        },
                _ =>    (), //self.ppuctrl = self.ppuctrl  // epic.
            };
        } else if address < 0x4020 {
            /* Apu AND IO TODO*/
            self.write_status = MemState::Io;
            match address {
                0x4000 =>   (),
                0x4001 =>   (),
                0x4002 =>   (),
                0x4003 =>   (),
                0x4004 =>   (),
                0x4005 =>   (),
                0x4006 =>   (),
                0x4007 =>   (),
                0x4008 =>   (),
                0x4009 =>   (),
                0x400A =>   (),
                0x400B =>   (),
                0x400C =>   (),
                0x400D =>   (),
                0x400E =>   (),
                0x400F =>   (),
                0x4010 =>   (),
                0x4011 =>   (),
                0x4012 =>   (),
                0x4013 =>   (),
                0x4014 =>       // When oamdma is written to
                                // the cpu locks down and fills the
                                // the oam memory with the selected page.
                                // (value in oamdma).
                            {   self.write_status = MemState::Oamdma;
                                self.dma = true;
                                self.oamdma = val
                            },
                0x4015 =>   (),
                0x4016 =>   (),
                0x4017 =>   (),
                0x4018 =>   (),
                0x4019 =>   (),
                0x401A =>   (),
                0x401B =>   (),
                0x401C =>   (),
                0x401D =>   (),
                0x401E =>   (), 
                0x401F =>   (),
                _      =>   (),
            }
        } else if address < 0x6000 {
            /* Cartridge expansion ROM the f */
            self.write_status = MemState::Memory;
        } else if address < 0x8000 {
            /* SRAM */
            self.write_status = MemState::Memory;
        } else /* 0x8000 <= address < 0xC000*/ {
            /* PRG-ROM */
            self.write_status = MemState::Memory;
        }
    }

    pub fn load_word(&mut self, address: W<u16>) -> W<u16> {
        let low = W16!(self.load(address));
        (W16!(self.load(address + W(1))) << 8) | low
    }

    pub fn store_word(&mut self, address: W<u16>, word: W<u16>) {
        self.store(address, W8!(word >> 8));
        self.store(address + W(1), W8!(word));
    }

    pub fn load_word_page_wrap(&mut self, address: W<u16>) -> W<u16> {
        let low = self.load(address);
        let high = self.load((address & PAGE_MASK) | W16!(W8!(address) + W(1)));
        (W16!(high) << 8) | W16!(low)
    }
}


impl fmt::Debug for Memory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut output = "ram: [".to_string();
        for i in 0..2047{
            output.push_str(&format!("{:#x}|", self.ram[i]));
        }
        output.push_str(&format!("{:#x}]", self.ram[2047]));
        write!(f, "{{ ppuctrl: {:#x}, ppumask: {:#x}, ppustatus: {:#x}, oamaddr: {:#x}, oamdata: {:#x}, ppuscroll: {:#x}, ppuaddr: {:#x}, ppudata: {:#x}, oamdma: {:#x}, read_status: {}, write_status: {}}}, \n {}", 
                      self.ppuctrl, self.ppumask, self.ppustatus, self.oamaddr, self.oamdata, self.ppuscroll, self.ppuaddr, 
                      self.ppudata, self.oamdma, self.read_status, self.write_status, output)
    }
}


