fn main() {
    slint_build::compile("ui/app.slint").expect("не удалось скомпилировать ui/app.slint");

    // Иконка и метаданные в ресурсах exe (Проводник, панель задач).
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        winresource::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .set("ProductName", "Домашняя аптечка")
            .set("FileDescription", "Учёт сроков годности лекарств")
            .compile()
            .expect("не удалось встроить иконку в exe");
    }
}
