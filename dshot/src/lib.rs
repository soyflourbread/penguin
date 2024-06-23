#![no_std]

use embassy_rp::gpio;

mod api;

pub struct PioDshot<'a, P: embassy_rp::pio::Instance, const SM: usize> {
    sm: embassy_rp::pio::StateMachine<'a, P, SM>,
}

impl<'a, P: embassy_rp::pio::Instance, const SM: usize> PioDshot<'a, P, SM> {
    pub fn new(
        common: &mut embassy_rp::pio::Common<'a, P>,
        mut sm: embassy_rp::pio::StateMachine<'a, P, SM>,
        pin: impl embassy_rp::pio::PioPin,
        clk_div: u16,
    ) -> Self {
        let prg = pio_proc::pio_asm!(
            r#"
            set pindirs, 1
            entry:
                pull
                out null 16
                set x 15 ; will be executed 16 times
            loop_entry:
                set pins 1
                out y 1
                jmp !y zero
                nop [2]
            one: ; 12 and 4
                set pins 0
                jmp x-- loop_entry
                jmp reset
            zero: ; 6 and 10
                set pins 0 [3]
                jmp x-- loop_entry
                jmp reset
            loop_end:
            reset:
                nop [31]
                nop [31]
                nop [31]
                jmp entry [31]
            "#
        );

        Self { sm }
    }
}
