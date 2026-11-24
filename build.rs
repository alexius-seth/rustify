fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/rustify.ico");
        res.set("ProductName", "Rustify");
        res.set("FileDescription", "Rustify Lightweight Audio Engine");
        res.compile().expect("Failed to compile Windows resources");
    }
}