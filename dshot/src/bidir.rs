use defmt::info;
use embassy_rp::{gpio, pio};
use embassy_time::{Instant, Timer};

use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;
use crate::{api, DshotTx};

pub struct PioDshot<'a, P: pio::Instance, const SM: usize> {
    sm: pio::StateMachine<'a, P, SM>,
    pin: pio::Pin<'a, P>,
    origin: u8,
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

            write_entry:
                out null 16
            write_start:
                nop side 0 [1] ; 2 cycles of low
                out pins 1 [3] ; 1 extra cycle of high and 3 cycles of out
                nop side 1 ; 1 cycle of high
            write_end:
                jmp !osre write_start ; 1 extra cycle of low

            in null 32

            ; frame sent, try receiving
            set pindirs 0

            read_entry:
                set x 19 ; executed 20 times
            read_guard:
                wait 0 pin 0
                in null 16 [7] ; first bit is always 0
            read_start:
                in pins 1 [1] ; get first bit
                in pins 1 [1] ; get second bit
                in pins 1 [1] ; get third bit
                in pins 1 ; get last bit
            read_end:
                jmp x-- read_start; 1 extra cycle of low

            set pindirs 1 [7]; restore pin to out
            nop side 1 [7]; push final bit and reset pin level
            "#
        );
        let prg = common.load_program(&prg.program);

        let mut pin = common.make_pio_pin(pin);
        pin.set_pull(gpio::Pull::Up);
        sm.set_pin_dirs(pio::Direction::Out, &[&pin]);
        sm.set_pins(gpio::Level::High, &[&pin]);

        let mut cfg = pio::Config::default();
        cfg.set_set_pins(&[&pin]);
        cfg.set_in_pins(&[&pin]);
        cfg.set_out_pins(&[&pin]);
        cfg.use_program(&prg, &[&pin]);
        cfg.shift_out = pio::ShiftConfig {
            auto_fill: true,
            threshold: 32,
            direction: pio::ShiftDirection::Left,
            ..Default::default()
        };
        cfg.shift_in = pio::ShiftConfig {
            auto_fill: true,
            threshold: 32,
            direction: pio::ShiftDirection::Left,
            ..Default::default()
        };
        let dshot_rate = 300u64 * 1000 * 8;
        cfg.clock_divider = (U56F8!(125_000_000) / dshot_rate).to_fixed();

        let origin = prg.origin;

        sm.set_config(&cfg);
        sm.set_enable(true);

        Self { sm, pin, origin }
    }
}

impl<'a, P: pio::Instance, const SM: usize> DshotTx for PioDshot<'a, P, SM> {
    type Output = Option<[u32; 3]>;

    async fn send_frame(&mut self, frame: u16) -> Self::Output {
        self.sm.tx().wait_push(!frame as u32).await;
        let status_tx = self.sm.rx().wait_pull().await;
        assert_eq!(status_tx, 0);

        Timer::after_micros(45).await; // 30 us + 4 bits = around 45 us
        if self.sm.rx().empty() {
            self.sm.restart();
            unsafe { pio::instr::exec_jmp(&mut self.sm, self.origin) }
            self.sm.set_pin_dirs(pio::Direction::Out, &[&self.pin]);
            return None; // state machine stuck
        }

        let ret = [
            self.sm.rx().wait_pull().await,
            self.sm.rx().wait_pull().await,
            self.sm.rx().wait_pull().await,
        ];
        info!("ret: {:#034b},{:#034b},{:#034b}", ret[0], ret[1], ret[2]);
        Some(ret)
    }

    async fn send_command(&mut self, command: api::Command) -> Self::Output {
        let command = command.try_into().unwrap();
        let frame = api::FrameBuilder::new(
            api::Frame { command, telemetry: true }
        ).invert().build();
        self.send_frame(frame).await
    }
}

