use utoipa::OpenApi;
fn main() {
    println!(
        "{}",
        rust_starterkit::ApiDoc::openapi()
            .to_pretty_json()
            .expect("serialize OpenAPI")
    );
}
