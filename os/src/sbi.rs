use sbi_rt::{NoReason, Shutdown, SystemFailure};

pub fn console_getchar() -> usize {
    #[allow(deprecated)]
    sbi_rt::legacy::console_getchar()
}

pub fn console_putchar(c: usize) {
    #[allow(deprecated)]
    sbi_rt::legacy::console_putchar(c);
}

pub fn set_timer(timer: usize) {
    sbi_rt::set_timer(timer as u64);
}

pub fn shutdown(failure: bool) -> ! {
    if !failure {
        sbi_rt::system_reset(Shutdown, NoReason);
    } else {
        sbi_rt::system_reset(Shutdown, SystemFailure);
    };

    unreachable!()
}
