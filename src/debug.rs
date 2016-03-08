/*// nes
use nes::Nes;

// std
use std::io;
use std::io::Error;
use std::path::Path;

// SDL2
use sdl2::event::Event;
use sdl2::keyboard::{Keycode};
use sdl2::{EventPump, Sdl};
use sdl2::render::Renderer;

macro_rules! rdbg {
    () => { "(rdbg)" }
}

// Macro to print undefined commands. It removes the newline.
macro_rules! undefinedc {
    ($input:expr) => (
        println!("{} Undefined command: {}. Try 'help'", rdbg!(),
                        $input[0..$input.len()-1].to_string());
    );
}

macro_rules! rnl {
    ($input:expr) => ($input[0..$input.len()-1]);
}

pub struct Debug  {
    nes: Nes,
}

impl Debug {
    pub fn load_rom<P: AsRef<Path>>(rom_path: P) -> Result<Debug, Error>   {
        let mut rnes = try!(Nes::load_rom(rom_path));
        Ok (
            Debug {
                nes : rnes,
            }
        )
    }
}

impl Debug {
    pub fn debug(&mut self) {
        let mut input : String = "".to_string();
        let mut stdin = io::stdin();
        'debug: loop {
            stdin.read_line(&mut input).unwrap();
            let words : Vec<&str>= input.split(" ").collect();
            if words.len() > 0 {
                println!("words[0] {}", &rnl!(words[0]));
                match &rnl!(words[0]) {
                    "l" => { println!("{} list", rdbg!()); }
                    // alone just one step
                    // with a number steps several instrs
                    "s" => { println!("{} step", rdbg!()); },
                    // Since we only have 6502 assembly
                    // all these commands are the same
                    "n"|"nexti"|"ni"|"stepi"|"si"
                        => { println!("{} next", rdbg!());
                             self.nes.cycle()
                             // Print executed instruction
                            },
                    "c" => { println!("{} continue", rdbg!()); },
                    "p" => { println!("{} print", rdbg!()); },
                    "b" => { println!("{} breakpoint", rdbg!());},
                    "q" => { break 'debug; },
                    _   => { undefinedc!(words[0]); },
                }
            }
        }
    }
}


*/
