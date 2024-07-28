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

    async fn send_frame(&mut self, frame: u16) -> Self::Output;
    async fn send_command(&mut self, command: api::Command) -> Self::Output;
    
    async fn arm(&mut self) {
        self.send_command(api::Command::Beep{ count: 1 }).await;
        Timer::after_millis(500).await;
        self.send_command(api::Command::MotorStop).await;
        Timer::after_millis(500).await;
    }
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
            .side_set 1 opt

            loop_entry:
                pull noblock
                mov x, osr
                out null 16 ; 3 cycles total
            loop_start:
                nop side 1 [2] ; 3 cycles of high
                out pins 1 [2]; 3 cycles of out
                nop side 0 ; 1 cycle of low
            loop_end:
                jmp !osre loop_start ; 1 extra cycle of low

            ; 256 cycles for a frame, 252 cycles required
            idle_entry:
                set y, 30 ; execute 31 times
            idle_start:
            idle_end:
                jmp y-- idle_start [7] ; 8 cycles x 31 = 248
                nop [3]
            "#
        );
        let mut pin = common.make_pio_pin(pin);
        pin.set_pull(gpio::Pull::Down);
        sm.set_pins(gpio::Level::Low, &[&pin]);
        sm.set_pin_dirs(pio::Direction::Out, &[&pin]);

        let mut cfg = pio::Config::default();
        cfg.set_set_pins(&[&pin]);
        cfg.set_out_pins(&[&pin]);
        cfg.use_program(&common.load_program(&prg.program), &[&pin]);
        cfg.shift_out = pio::ShiftConfig {
            threshold: 32,
            direction: pio::ShiftDirection::Left,
            ..Default::default()
        };
        let dshot_rate = 300u64 * 1000 * 8;
        cfg.clock_divider = (U56F8!(125_000_000) / dshot_rate).to_fixed();
        sm.set_config(&cfg);
        sm.tx().push(u32::MIN);
        sm.set_enable(true);

        Self { sm }
    }
}

impl<'a, P: pio::Instance, const SM: usize> DshotTx for PioDshot<'a, P, SM> {
    type Output = ();
    
    async fn send_frame(&mut self, frame: u16) {
        self.sm.tx().wait_push(frame as u32).await;
    }

    async fn send_command(&mut self, command: api::Command) {
        let command = command.try_into().unwrap();
        let frame = api::FrameBuilder::new(
            api::Frame { command, telemetry: false }
        ).build();
        self.send_frame(frame).await;
    }
}
