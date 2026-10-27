#[tokio::main(worker_threads = 2)]
async fn main() {
    if tezbar_extension_sdk::serve(tezbar_ip_geolocation::Adapter::production())
        .await
        .is_err()
    {
        eprintln!("IP Geolocation protocol terminated");
        std::process::exit(1);
    }
}
