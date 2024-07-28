#![no_std]
#![no_main]

use core::fmt::Write;
use heapless::String;

use embassy_executor::Spawner;
use embassy_rp::{adc, bind_interrupts, pwm};
use embassy_rp::{peripherals, pio};
use embassy_time::{Duration, Ticker};

use defmt::info;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => adc::InterruptHandler;
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = penguin_exp::blinker::Blinker::new(p.PIN_25, Duration::from_millis(100));

    let mut pwm_0_config: pwm::Config = Default::default();
    pwm_0_config.phase_correct = true;

    let divider = 0x20u8;

    let mut top = 125000000f32; // frequency of rp2040
    top *= 20.0;
    top /= 1000.0; // number of cycles for 20ms
    top /= 2.0; // phase correct wave
    top /= divider as f32;
    top -= 1.0;
    let top = top as u16;
    info!("pwm cycle: {}", top);
    pwm_0_config.divider = divider.into();
    pwm_0_config.top = top;
    pwm_0_config.compare_a = 0x0000;
    let pwm_0 = pwm::Pwm::new_output_a(p.PWM_SLICE3, p.PIN_22, pwm_0_config.clone());

    let mut ticker = Ticker::every(Duration::from_millis(400));
    let mut frame = String::<128>::new();
    while let () = ticker.next().await {
        frame.clear();

        led.blink().await;
    }
}
