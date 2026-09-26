fn main() {
    if let Err(error) = verslot::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
