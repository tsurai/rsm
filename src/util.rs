use std::os::unix::io::AsRawFd;
use std::io;
use libc;

pub fn is_a_tty() -> bool {
    let reader = io::stdin();
    unsafe { libc::isatty(reader.as_raw_fd()) == 1 }
}
