#![no_std]
#![no_main]

use penguin_dshot::DshotTx;

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use heapless::String;

use embassy_executor::Spawner;
use embassy_rp::{adc, bind_interrupts, gpio};
use embassy_rp::{peripherals, pio};
use embassy_time::{Duration, Ticker, Timer};
use static_cell::StaticCell;

use defmt::{info, unwrap};
use embassy_rp::gpio::Pin;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC_IRQ_FIFO => adc::InterruptHandler;
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
});

static THROTTLE: AtomicU16 = AtomicU16::new(0);

static PIO_0: StaticCell<peripherals::PIO0> = StaticCell::new();

#[embassy_executor::task]
async fn button_task(pin: gpio::AnyPin, mut esc_0: penguin_dshot::PioDshot<'static, peripherals::PIO0, 1>) {
    let input = gpio::Input::new(pin, gpio::Pull::Up);
    let mut button = penguin_exp::button::Button::new(input, Duration::from_millis(40));
    esc_0.entry();
    while button.debounce().await != gpio::Level::High {}
    let mut ticker = Ticker::every(Duration::from_millis(80));
    let mut throttle: f32 = THROTTLE.load(Ordering::Relaxed) as f32;
    loop {
        ticker.next().await;
        throttle *= 0.9;
        throttle += THROTTLE.load(Ordering::Relaxed) as f32 * 0.1;
        esc_0.send_command(penguin_dshot::api::Command::Throttle(throttle as u16));
    }
}

fn to_throttle(voltage: f32) -> u16 {
    let mut ret: f32 = 12.0; // max 12 volts
    ret /= voltage;
    ret *= 240.0; // base throttle
    ret as u16
}

#[embassy_executor::main]
async fn main(spawner: Spawner) { 
    let p = embassy_rp::init(Default::default());

    info!("init");

    // let servo_0 = penguin_exp::servo::ServoAB::new(
    //     p.PWM_SLICE1, p.PIN_18, p.PIN_19,
    //     20.0, 1.2, 1.8, 0.5
    // ); // datasheet: 0.5ms-2.5ms

    let pio_0: &'static mut _ = PIO_0.init(p.PIO0);
    let pio::Pio {
        mut common,
        sm0,
        sm1,
        ..
    } = pio::Pio::new(pio_0, Irqs);
    let mut uart_0 = penguin_exp::uart::PioUartTx::new(&mut common, sm0, p.PIN_0, 9600);
    let esc_0 = penguin_dshot::PioDshot::new(&mut common, sm1, p.PIN_2);
    let pin_btn = p.PIN_7.degrade();
    unwrap!(spawner.spawn(button_task(pin_btn, esc_0)));
    
    let mut adc = adc::Adc::new(p.ADC, Irqs, adc::Config::default());
    let mut potentiometer = penguin_exp::potentiometer::Potentiometer::new(p.PIN_29);
    let mut ticker = Ticker::every(Duration::from_millis(40));
    let mut frame: String<128> = String::new();
    loop {
        ticker.next().await;
        let vol = potentiometer.voltage(&mut adc).await.unwrap();
        THROTTLE.store(to_throttle(vol), Ordering::Relaxed);
        // frame.clear();
        // let _ = write!(frame, "vol: {}, temp: {} \r\n", vol, temp);
        // {
        //     use embedded_io_async::Write;
        //     uart_0.write(frame.as_bytes()).await.unwrap();
        // }
    }
}
