#![no_std]

use embassy_rp::gpio;
use embassy_time::Timer;

use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

pub mod api;

pub trait DshotTx {
    async fn send_frame(&mut self, frame: u16);
    async fn send_command(&mut self, command: api::Command) {
        let command = command.try_into().unwrap();
        let frame = api::FrameBuilder::new(
            api::Frame { command, telemetry: false }
        ).build();
        self.send_frame(frame).await;
    }
    
    async fn arm(&mut self) {
        for _i in 0..2000 {
            self.send_command(api::Command::MotorStop).await;
            Timer::after_millis(1).await;
        }
        for _i in 0..1000 {
            self.send_command(api::Command::Reverse(false)).await;
            Timer::after_millis(1).await;
        }
    }
}

pub struct PioDshot<'a, P: embassy_rp::pio::Instance, const SM: usize> {
    sm: embassy_rp::pio::StateMachine<'a, P, SM>,
}

impl<'a, P: embassy_rp::pio::Instance, const SM: usize> PioDshot<'a, P, SM> {
    pub fn new(
        common: &mut embassy_rp::pio::Common<'a, P>,
        mut sm: embassy_rp::pio::StateMachine<'a, P, SM>,
        pin: impl embassy_rp::pio::PioPin,
    ) -> Self {
        // 6:2 for high, 3:5 for low
        let prg = pio_proc::pio_asm!(
            r#"
            .side_set 1 opt

            loop_entry:
                out null 16
            loop_start:
                out x 1 side 1 [1]
                jmp !x falling_edge ; 3 cycles of high so far
                nop [2] ; 3 additional cycles of high
            falling_edge:
                jmp x-- loop_end side 0 ; 1 cycle of low so far
                nop [2] ; 3 additional cycles of low
            loop_end:
                jmp !osre loop_start ; 1 extra cycle of low

            reset_entry: ; delay 128 cycles (why)
                set x 14 [7] ; delay 8 cycles
            reset_start:
            reset_end:
                jmp x-- reset_start [7] ; delay 8 cycles
            "#
        );
        let pin = common.make_pio_pin(pin);
        sm.set_pins(gpio::Level::Low, &[&pin]);
        sm.set_pin_dirs(embassy_rp::pio::Direction::Out, &[&pin]);

        let mut cfg = embassy_rp::pio::Config::default();
        cfg.set_set_pins(&[&pin]);
        cfg.use_program(&common.load_program(&prg.program), &[&pin]);
        cfg.shift_out = embassy_rp::pio::ShiftConfig {
            auto_fill: true,
            threshold: 32,
            direction: embassy_rp::pio::ShiftDirection::Left,
            ..Default::default()
        };
        let dshot_rate = 300u64 * 1000 * 8;
        cfg.clock_divider = (U56F8!(125_000_000) / dshot_rate).to_fixed();
        sm.set_config(&cfg);
        sm.set_enable(true);

        Self { sm }
    }
}

impl<'a, P: embassy_rp::pio::Instance, const SM: usize> DshotTx for PioDshot<'a, P, SM> {
    async fn send_frame(&mut self, frame: u16) {
        self.sm.tx().wait_push(frame as u32).await;
    }
}
