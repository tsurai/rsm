use std::os::unix::io::AsRawFd;
use std::{io, cmp};
use snippet::Snippet;
use time;
use libc;

pub fn is_a_tty() -> bool {
    let reader = io::stdin();
    unsafe { libc::isatty(reader.as_raw_fd()) == 1 }
}

pub fn get_list_col_widths(snippets: &Vec<Snippet>) -> (usize, usize, usize) {
    snippets.iter().fold((2, 4, 4), |acc, x| {
        (cmp::max(acc.0, x.id.to_string().len()), cmp::max(acc.1, x.tags.as_slice().join(", ").len()), cmp::max(acc.2, x.name.len()))
    })
}

pub fn get_utc_now() -> i64 {
    time::now_utc().to_timespec().sec
}
