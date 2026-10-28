fn main() {
    if let Err(error) = verslot::run() {
        verslot::print_error(&error);
        std::process::exit(1);
    }
}
