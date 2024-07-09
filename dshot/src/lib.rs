#![no_std]

pub mod api;
pub mod bidir;

use embassy_rp::{gpio, pio};
use embassy_time::{Duration, Ticker, Timer};

use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

pub trait DshotTx {
    type Output;

    async fn send_frame(&mut self, frame: u16) -> Self::Output;
    async fn send_command(&mut self, command: api::Command) -> Self::Output;
    
    async fn arm(&mut self) {
        let mut ticker = Ticker::every(Duration::from_micros(4000));
        for _i in 0..500 {
            self.send_command(api::Command::MotorStop).await;
            ticker.next().await;
        }
        for _i in 0..200 {
            self.send_command(api::Command::Reverse(false)).await;
            ticker.next().await;
        }
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
                out null 16
            loop_start:
                nop side 1 [1] ; 2 cycles of high
                out pins 1 [3]; 1 extra cycle of high and 3 cycles of out
                nop side 0 ; 1 cycle of low
            loop_end:
                jmp !osre loop_start ; 1 extra cycle of low
            "#
        );
        let pin = common.make_pio_pin(pin);
        sm.set_pins(gpio::Level::Low, &[&pin]);
        sm.set_pin_dirs(pio::Direction::Out, &[&pin]);

        let mut cfg = pio::Config::default();
        cfg.set_set_pins(&[&pin]);
        cfg.set_out_pins(&[&pin]);
        cfg.use_program(&common.load_program(&prg.program), &[&pin]);
        cfg.shift_out = pio::ShiftConfig {
            auto_fill: true,
            threshold: 32,
            direction: pio::ShiftDirection::Left,
            ..Default::default()
        };
        let dshot_rate = 300u64 * 1000 * 8;
        cfg.clock_divider = (U56F8!(125_000_000) / dshot_rate).to_fixed();
        sm.set_config(&cfg);
        sm.set_enable(true);

        Self { sm }
    }
}

impl<'a, P: pio::Instance, const SM: usize> DshotTx for PioDshot<'a, P, SM> {
    type Output = ();
    
    async fn send_frame(&mut self, frame: u16) {
        self.sm.tx().wait_push(frame as u32).await;
        Timer::after_micros(240).await;
    }

    async fn send_command(&mut self, command: api::Command) {
        let command = command.try_into().unwrap();
        let frame = api::FrameBuilder::new(
            api::Frame { command, telemetry: false }
        ).build();
        self.send_frame(frame).await;
    }
}
