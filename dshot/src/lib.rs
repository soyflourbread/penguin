#![no_std]

pub mod api;
pub mod bidir;

use defmt::info;
use embassy_rp::{gpio, pio};
use embassy_time::{Duration, Ticker, Timer};

use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

pub trait DshotTx {
    type Output;

    fn entry(&mut self);
    fn send_frame(&mut self, frame: u16);
    fn send_command(&mut self, command: api::Command);
    
    fn drain(&mut self) -> Self::Output;
}

pub struct PioDshot<'a, P: pio::Instance, const SM: usize> {
    sm: pio::StateMachine<'a, P, SM>,
}

impl<'a, P: pio::Instance, const SM: usize> PioDshot<'a, P, SM> {
    pub fn new(
        common: &mut pio::Common<'a, P>,
        mut sm: pio::StateMachine<'a, P, SM>,
        pin: impl pio::PioPin,
    ) -> Self {
        // 6:2 for high, 3:5 for low
        let prg = pio_proc::pio_asm!(
            r#"
            loop_entry:
                pull noblock
                mov x, osr
                out null 16 ; 3 cycles total
            loop_start:
                set pins 1 [14] ; 15 cycles of high
                out pins 1 [14] ; 15 cycles of out
                set pins 0 [8] ; 9 cycle of low
            loop_end:
                jmp !osre loop_start ; 1 extra cycle of low

            ; 640 cycles for a frame, 637 cycles required
            idle_entry:
                set y, 18 [28]; execute 19 times, 29 cycles
            idle_start:
            idle_end:
                jmp y-- idle_start [31] ; 32 cycles x 19 = 608
            "#
        );
        let mut pin = common.make_pio_pin(pin);
        pin.set_pull(gpio::Pull::Down);
        sm.set_pins(gpio::Level::Low, &[&pin]);
        sm.set_pin_dirs(pio::Direction::Out, &[&pin]);

        let mut cfg = pio::Config::default();
        cfg.set_set_pins(&[&pin]);
        cfg.set_out_pins(&[&pin]);
        cfg.use_program(&common.load_program(&prg.program), &[]);
        cfg.shift_out = pio::ShiftConfig {
            threshold: 32,
            direction: pio::ShiftDirection::Left,
            ..Default::default()
        };
        let dshot_rate = 300u64 * 1000 * 8 * 5; // 40 cycles per bit
        cfg.clock_divider = (U56F8!(125_000_000) / dshot_rate).to_fixed();
        sm.set_config(&cfg);
        sm.tx().push(u32::MIN);
        Self { sm }
    }
}

impl<'a, P: pio::Instance, const SM: usize> DshotTx for PioDshot<'a, P, SM> {
    type Output = ();

    fn entry(&mut self) {
        self.sm.set_enable(true);
    }

    fn send_frame(&mut self, frame: u16) {
        self.sm.tx().push(frame as u32);
    }

    fn send_command(&mut self, command: api::Command) {
        let command = command.try_into().unwrap();
        let frame = api::FrameBuilder::new(
            api::Frame { command, telemetry: false }
        ).build();
        self.send_frame(frame);
    }
    
    fn drain(&mut self) {}
}
