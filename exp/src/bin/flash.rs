#![no_std]
#![no_main]

use core::fmt::Write;
use heapless::String;

use embassy_executor::Spawner;
use embassy_rp::{adc, bind_interrupts, gpio, pwm};
use embassy_rp::{pio, i2c, peripherals, uart};
use embassy_time::{Duration, Ticker};
use static_cell::StaticCell;

use defmt::{info, unwrap};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => adc::InterruptHandler;
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
});

static PIO_0: StaticCell<peripherals::PIO0> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // let servo_0 = penguin_exp::servo::ServoAB::new(
    //     p.PWM_SLICE1, p.PIN_18, p.PIN_19,
    //     20.0, 1.2, 1.8, 0.5
    // ); // datasheet: 0.5ms-2.5ms

    let pio_0: &'static mut _ = PIO_0.init(p.PIO0);
    let pio::Pio {
        mut common, sm0, sm1, ..
    } = pio::Pio::new(pio_0, Irqs);
    // let mut uart_0 = penguin_exp::uart::PioUartTx::new(
    //     &mut common, sm0,
    //     p.PIN_16, 9600,
    // );
    let mut bl_0 = penguin_exp::blheli_passthrough::BlHeliPassThrough::new(
        &mut common, sm0,
        p.PIN_22
    );

    let mut led = penguin_exp::blinker::Blinker::new(
        p.PIN_25,
        Duration::from_millis(100),
    );

    for _ in 0..3 {
        led.blink().await;
    }

    // loop {
    //     let frame = bl_0.read_u8().await;
    //     info!("frame: {}", frame);
    // }
}
