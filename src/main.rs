use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pixels::{Pixels, SurfaceTexture};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const FONT_START: usize = 0x0;
const FONTSET: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

const WIDTH: u32 = 64;
const HEIGHT: u32 = 32;
const MEMORY_SIZE: usize = 4096;

#[derive(Debug, Default)]
struct Chip8 {
    cpu: Cpu,
    memory: Memory,
    timer: Timer,
    keyboard: Keyboard,
    display: Display,
}

impl Chip8 {
    fn load_program(&mut self, program: &[u8]) {
        self.memory.load_program(program);
    }

    fn cycle(&mut self) -> Result<(), &'static str> {
        let opcode = self.fetch_opcode();
        self.cpu.pc += 2;

        self.execute_opcode(opcode)?;

        Ok(())
    }

    fn fetch_opcode(&self) -> u16 {
        let pc = self.cpu.pc as usize;

        let high = self.memory.values[pc];
        let low = self.memory.values[pc + 1];

        ((high as u16) << 8) | low as u16
    }

    fn execute_opcode(&mut self, opcode: u16) -> Result<(), &'static str> {
        match opcode & 0xF000 {
            0x0000 => match opcode {
                // 00E0 - CLS: Clear display
                0x00E0 => self.display.clear(),
                // 00EE - RET: Return from a subroutine
                0x00EE => {
                    self.cpu.pc = self.cpu.stack.pop()?;
                }
                _ => {}
            },
            // 1nnn - JP addr: Jump to address nnn
            0x1000 => {
                let nnn = opcode & 0x0FFF;
                self.cpu.pc = nnn;
            }
            // 2nnn - CALL addr: Call subroutine at nnn.
            0x2000 => {
                let nnn = opcode & 0x0FFF;
                self.cpu.stack.push(self.cpu.pc)?;
                self.cpu.pc = nnn;
            }
            // 3xkk - SE Vx, byte: Skip next instruction if Vx = kk
            0x3000 => {
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let kk = opcode & 0x00FF;

                if self.cpu.registers[x] == kk as u8 {
                    self.cpu.pc += 2;
                }
            }
            // 4xkk - SNE Vx, byte: Skip next instruction if Vx != kk
            0x4000 => {
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let kk = opcode & 0x00FF;

                if self.cpu.registers[x] != kk as u8 {
                    self.cpu.pc += 2;
                }
            }
            // 5xy0 - SE Vx, Vy: Skip next instruction if Vx = Vy
            0x5000 => {
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let y = ((opcode & 0x00F0) >> 4) as usize;
                let vx = self.cpu.registers[x];
                let vy = self.cpu.registers[y];

                if vx == vy {
                    self.cpu.pc += 2;
                }
            }
            // 6xkk - LD Vx, byte: Set Vx = kk
            0x6000 => {
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let kk = opcode & 0x00FF;

                self.cpu.registers[x] = kk as u8;
            }
            // 7xkk - ADD Vx, byte: Set Vx = Vx + kk
            0x7000 => {
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let kk = (opcode & 0x00FF) as u8;
                self.cpu.registers[x] = self.cpu.registers[x].wrapping_add(kk);
            }
            0x8000 => match opcode & 0xF00F {
                // 8xy0 - LD Vx, Vy: Set Vx = Vy
                0x8000 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    self.cpu.registers[x] = self.cpu.registers[y];
                }
                // 8xy1 - OR Vx, Vy: Set Vx = Vx OR Vy
                0x8001 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    let vx = self.cpu.registers[x];
                    let vy = self.cpu.registers[y];
                    self.cpu.registers[x] = vx | vy;
                }
                // 8xy2 - AND Vx, Vy: Set Vx = Vx AND Vy
                0x8002 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    let vx = self.cpu.registers[x];
                    let vy = self.cpu.registers[y];
                    self.cpu.registers[x] = vx & vy;
                }
                // 8xy3 - XOR Vx, Vy: Set Vx = Vx XOR Vy
                0x8003 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    let vx = self.cpu.registers[x];
                    let vy = self.cpu.registers[y];
                    self.cpu.registers[x] = vx ^ vy;
                }
                // 8xy4 - ADD Vx, Vy: Set Vx = Vx + Vy, set VF = carry
                0x8004 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    let vx = self.cpu.registers[x];
                    let vy = self.cpu.registers[y];
                    let (result, carry) = vx.overflowing_add(vy);
                    self.cpu.registers[x] = result;
                    self.cpu.registers[0xF] = if carry { 1 } else { 0 };
                }
                // 8xy5 - SUB Vx, Vy: Set Vx = Vx - Vy, set VF = NOT borrow.
                0x8005 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    let vx = self.cpu.registers[x];
                    let vy = self.cpu.registers[y];
                    let (result, borrow) = vx.overflowing_sub(vy);
                    self.cpu.registers[x] = result;
                    self.cpu.registers[0xF] = if borrow { 0 } else { 1 };
                }
                // 8xy6 - SHR Vx {, Vy}: Set Vx = Vx SHR 1
                0x8006 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;

                    let lsb = self.cpu.registers[x] & 1;
                    self.cpu.registers[x] >>= 1;
                    self.cpu.registers[0xF] = lsb;
                }
                // 8xy7 - SUBN Vx, Vy: Set Vx = Vy - Vx, set VF = NOT borrow.
                0x8007 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    let vx = self.cpu.registers[x];
                    let vy = self.cpu.registers[y];
                    let (result, borrow) = vy.overflowing_sub(vx);
                    self.cpu.registers[x] = result;
                    self.cpu.registers[0xF] = if borrow { 0 } else { 1 };
                }
                // 8xyE - SHL Vx {, Vy}: Set Vx = Vx SHL 1
                0x800E => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;

                    let msb = (self.cpu.registers[x] & 0x80) >> 7;
                    self.cpu.registers[x] <<= 1;
                    self.cpu.registers[0xF] = msb;
                }
                _ => {}
            },
            // 9xy0 - SNE Vx, Vy: Skip next instruction if Vx != Vy
            0x9000 => {
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let y = ((opcode & 0x00F0) >> 4) as usize;

                let vx = self.cpu.registers[x];
                let vy = self.cpu.registers[y];

                if vx != vy {
                    self.cpu.pc += 2;
                }
            }
            // Annn - LD I, addr: Set I = nnn
            0xA000 => {
                let nnn = opcode & 0x0FFF;
                self.cpu.i = nnn;
            }
            // Bnnn - JP V0, addr: Jump to location nnn + V0
            0xB000 => {
                let nnn = opcode & 0x0FFF;
                self.cpu.pc = nnn.wrapping_add(self.cpu.registers[0x0] as u16);
            }
            // Cxkk - RND Vx, byte: Set Vx = random byte AND kk
            0xC000 => {
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let kk = (opcode & 0x00FF) as u8;

                let random: u8 = rand::random();

                self.cpu.registers[x] = random & kk;
            }
            // Dxyn - DRW Vx, Vy, nibble
            0xD000 => {
                let vx = self.cpu.registers[((opcode & 0x0F00) >> 8) as usize];
                let vy = self.cpu.registers[((opcode & 0x00F0) >> 4) as usize];
                let n = (opcode & 0x000F) as usize;

                self.cpu.registers[0x0F] = 0;

                for row in 0..n {
                    let sprite = self.memory.values[(self.cpu.i as usize + row) % MEMORY_SIZE];

                    for col in 0..8 {
                        if sprite & (0x80 >> col) != 0 {
                            let px = (vx.wrapping_add(col)) % Display::WIDTH;
                            let py = (vy.wrapping_add(row as u8)) % Display::HEIGHT;

                            if self.display.toggle_pixel(px, py) {
                                self.cpu.registers[0x0F] = 1;
                            }
                        }
                    }
                }
            }
            0xE000 => match opcode & 0xF0FF {
                // Ex9E - SKP Vx: Skip next instruction if key with the value of Vx is pressed
                0xE09E => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let key = (self.cpu.registers[x] & 0xF) as usize;
                    if self.keyboard.keys[key] {
                        self.cpu.pc += 2;
                    }
                }
                // ExA1 - SKNP Vx: Skip next instruction if key with the value of Vx is not pressed
                0xE0A1 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let key = (self.cpu.registers[x] & 0xF) as usize;
                    if !self.keyboard.keys[key] {
                        self.cpu.pc += 2;
                    };
                }
                _ => {}
            },
            0xF000 => match opcode & 0xF0FF {
                // Fx07 - LD Vx, DT: Set Vx = delay timer value
                0xF007 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    self.cpu.registers[x] = self.timer.dt;
                }
                // Fx0A - LD Vx, K: Wait for a key press, store the value of the key in Vx
                0xF00A => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let Some(key) = self.keyboard.just_pressed() else {
                        self.cpu.pc -= 2;
                        return Ok(());
                    };

                    self.cpu.registers[x] = key;
                }
                // Fx15 - LD DT, Vx: Set delay timer = Vx
                0xF015 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let vx = self.cpu.registers[x];
                    self.timer.dt = vx;
                }
                // Fx18 - LD ST, Vx: Set sound timer = Vx
                0xF018 => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let vx = self.cpu.registers[x];
                    self.timer.st = vx;
                }
                // Fx1E - ADD I, Vx: Set I = I + Vx.
                0xF01E => {
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let vx = self.cpu.registers[x];

                    self.cpu.i = self.cpu.i.wrapping_add(vx as u16);
                }
                // Fx29 - LD F, Vx: Set I = location of sprite for digit Vx.
                0xF029 => {
                    let x = ((opcode & 0xF00) >> 8) as usize;
                    let vx = self.cpu.registers[x] as usize;

                    self.cpu.i = (FONT_START + vx * 5) as u16;
                }
                // Fx33 - LD B, Vx: Store BCD representation of Vx in memory locations I, I+1, and I+2
                0xF033 => {
                    let x = ((opcode & 0xF00) >> 8) as usize;
                    let vx = self.cpu.registers[x];
                    let hundreds_digit = (vx / 100) % 10;
                    let tens_digit = (vx / 10) % 10;
                    let ones_digit = vx % 10;

                    self.memory.values[(self.cpu.i as usize) % MEMORY_SIZE] = hundreds_digit;
                    self.memory.values[(self.cpu.i as usize + 1) % MEMORY_SIZE] = tens_digit;
                    self.memory.values[(self.cpu.i as usize + 2) % MEMORY_SIZE] = ones_digit;
                }
                // Fx55 - LD [I], Vx: Store registers V0 through Vx in memory starting at location I
                0xF055 => {
                    let x = ((opcode & 0xF00) >> 8) as usize;
                    let i = self.cpu.i as usize;

                    for register in 0..=x {
                        self.memory.values[(i + register) % MEMORY_SIZE] =
                            self.cpu.registers[register]
                    }
                }
                // Fx65 - LD Vx, [I]: Read registers V0 through Vx from memory starting at location I
                0xF065 => {
                    let x = ((opcode & 0xF00) >> 8) as usize;
                    let i = self.cpu.i as usize;

                    for register in 0..=x {
                        self.cpu.registers[register] =
                            self.memory.values[(i + register) % MEMORY_SIZE];
                    }
                }
                _ => {}
            },
            _ => panic!("Unknown CHIP-8 opcode encountered: {:#06X}", opcode),
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Cpu {
    registers: [u8; 16],
    pc: u16,
    i: u16,
    stack: Stack,
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            registers: [0; 16],
            pc: 0x200,
            i: 0,
            stack: Stack::default(),
        }
    }
}

#[derive(Debug, Default)]
struct Stack {
    values: [u16; 16],
    sp: u8,
}

impl Stack {
    fn push(&mut self, value: u16) -> Result<(), &'static str> {
        if self.sp as usize >= self.values.len() {
            return Err("Stack overflow: attempted to push value when stack is full");
        }
        self.values[(self.sp) as usize] = value;
        self.sp += 1;

        Ok(())
    }

    fn pop(&mut self) -> Result<u16, &'static str> {
        if self.sp == 0 {
            return Err("Stack underflow: attempted to return with an empty stack");
        }
        self.sp -= 1;
        Ok(self.values[self.sp as usize])
    }
}

#[derive(Debug)]
struct Memory {
    pub values: [u8; 4096],
}

impl Memory {
    fn load_program(&mut self, program: &[u8]) {
        self.values[0x200..0x200 + program.len()].copy_from_slice(program)
    }
}

impl Default for Memory {
    fn default() -> Self {
        let mut values = [0; 4096];

        for (i, byte) in FONTSET.iter().enumerate() {
            values[FONT_START + i] = *byte;
        }

        Self { values }
    }
}

#[derive(Debug, Default)]
struct Timer {
    dt: u8,
    st: u8,
}

#[derive(Debug, Default)]
struct Keyboard {
    keys: [bool; 16],
    pressed_this_frame: Option<u8>,
}

impl Keyboard {
    pub fn just_pressed(&mut self) -> Option<u8> {
        self.pressed_this_frame.take()
    }
}

#[derive(Debug, Default)]
struct Display {
    pixels: [u64; 32],
}

impl Display {
    const WIDTH: u8 = 64;
    const HEIGHT: u8 = 32;

    fn clear(&mut self) {
        self.pixels = [0; 32];
    }

    fn get_pixel(&self, x: u8, y: u8) -> bool {
        let x = x % Self::WIDTH;
        let y = y % Self::HEIGHT;

        (self.pixels[y as usize] & (1 << x)) != 0
    }

    pub fn toggle_pixel(&mut self, x: u8, y: u8) -> bool {
        let x = x % Self::WIDTH;
        let y = y % Self::HEIGHT;

        let mask = 1 << x;

        let collision = (self.pixels[y as usize] & mask) != 0;

        self.pixels[y as usize] ^= mask;

        collision
    }
}

const FRAME: Duration = Duration::from_micros(16_667);
const CYCLES_PER_FRAME: usize = 10;

struct App {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    chip8: Chip8,
    last_tick: Instant,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("CHIP-8 Emulator"))
                .unwrap(),
        );

        let size = window.inner_size();
        let surface_texture = SurfaceTexture::new(size.width, size.height, window.clone());
        let pixels = Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap();

        self.window = Some(window);
        self.pixels = Some(pixels);
        self.last_tick = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let (Some(_window), Some(pixels)) = (&self.window, &mut self.pixels) else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                pixels.resize_surface(size.width, size.height).unwrap();
            }
            WindowEvent::RedrawRequested => {
                let frame = pixels.frame_mut();
                for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
                    let x = (i % WIDTH as usize) as u8;
                    let y = (i / WIDTH as usize) as u8;
                    let v = if self.chip8.display.get_pixel(x, y) {
                        0xFF
                    } else {
                        0x00
                    };
                    pixel.copy_from_slice(&[v, v, v, 0xFF]);
                }

                if let Err(e) = pixels.render() {
                    eprintln!("render error: {e}");
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.last_tick.elapsed() < FRAME {
            return;
        }
        self.last_tick = Instant::now();

        for _ in 0..CYCLES_PER_FRAME {
            if let Err(e) = self.chip8.cycle() {
                eprintln!("emulation stopped: {e}");
                event_loop.exit();
                return;
            }
        }
        self.chip8.timer.dt = self.chip8.timer.dt.saturating_sub(1);
        self.chip8.timer.st = self.chip8.timer.st.saturating_sub(1);

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    let rom = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tests/ibm-logo.ch8".to_string());
    let program = std::fs::read(&rom).unwrap_or_else(|e| panic!("failed to read {rom}: {e}"));

    let mut chip8 = Chip8::default();
    chip8.load_program(&program);

    let event_loop = EventLoop::new().unwrap();
    let mut app = App {
        window: None,
        pixels: None,
        chip8,
        last_tick: Instant::now(),
    };
    event_loop.run_app(&mut app).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_rom(bytes: &[u8]) -> Chip8 {
        let mut c = Chip8::default();
        c.load_program(bytes);
        for _ in 0..500_000 {
            c.cycle().unwrap();
        }
        c
    }

    #[test]
    fn corax() {
        let c = run_rom(include_bytes!("../tests/corax+.ch8"));
        insta::assert_debug_snapshot!(c.display.pixels);
    }

    #[test]
    fn flags() {
        let c = run_rom(include_bytes!("../tests/flags.ch8"));
        insta::assert_debug_snapshot!(c.display.pixels);
    }
}
