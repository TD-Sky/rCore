use core::panic::PanicInfo;

use crate::process::getpid;
use crate::signal::{SIGABRT, kill};

#[panic_handler]
fn panic_handler(panic_info: &PanicInfo) -> ! {
    let err = panic_info.message();

    if let Some(location) = panic_info.location() {
        println!(
            "Panicked at {}:{}, {}",
            location.file(),
            location.line(),
            err
        );
    } else {
        println!("Panicked: {}", err);
    }

    kill(getpid(), SIGABRT);
    unreachable!()
}
