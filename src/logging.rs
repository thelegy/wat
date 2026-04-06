const RESET: &str = "\x1b[0m";
const BLUE: &str = "\x1b[34;1m";
const YELLOW: &str = "\x1b[33;1m";
const RED: &str = "\x1b[31;1m";

pub fn print_info(message: impl AsRef<str>) {
    eprintln!("{BLUE}{}{RESET}", message.as_ref());
}

pub fn print_warning(message: impl AsRef<str>) {
    eprintln!("{YELLOW}{}{RESET}", message.as_ref());
}

pub fn print_error(message: impl AsRef<str>) {
    eprintln!("{RED}{}{RESET}", message.as_ref());
}
