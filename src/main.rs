use tracing::error;

fn main() {
    if let Err(e) = mqttserver::get_args().and_then(mqttserver::run) {
        error!("server binary exit {}", e);
        std::process::exit(1);
    }
}
