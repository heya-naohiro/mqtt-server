fn main() {
    if let Err(e) = mqttserver::get_args().and_then(mqttserver::run) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
