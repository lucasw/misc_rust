#![no_main]
#![no_std]

use cortex_m_semihosting::hprintln;
use embassy_executor::{Spawner, main, task};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::Timer;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    hprintln!("{}", info);
    loop {}
}

#[main]
async fn main(spawner: Spawner) {
    hprintln!("embassy led flashing start");

    let p = embassy_stm32::init(Default::default());
    /*
    let mut led_green = pb0.into_push_pull_output();
    let mut led_orange = gpioe.pe1.into_push_pull_output();
    let mut led_red = gpiob.pb14.into_push_pull_output();
    */
    let led_green = Output::new(p.PB0, Level::Low, Speed::Medium);
    let led_orange = Output::new(p.PE1, Level::Low, Speed::Medium);
    let led_red = Output::new(p.PB14, Level::High, Speed::Medium);

    spawner.must_spawn(flash_led(led_green, 500));
    spawner.must_spawn(flash_led(led_orange, 1100));
    spawner.must_spawn(flash_led(led_red, 2250));
}

#[task(pool_size = 3)]
async fn flash_led(mut gpio: Output<'static>, half_period: u64) -> ! {
    loop {
        gpio.set_high();
        Timer::after_millis(half_period).await;
        gpio.set_low();
        Timer::after_millis(half_period).await;
    }
}
