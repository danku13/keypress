//! Сборка keypress.
//!
//! Обычная сборка (`cargo build --release`) — стандартная, манифест не
//! вшивается, поведение не меняется.
//!
//! UIAccess-вариант (оверлей НАД меню Пуск на Win11) собирается так:
//!
//!     KEYPRESS_UIACCESS=1 cargo build --release
//!
//! Тогда в exe вшивается манифест assets/keypress-uiaccess.manifest
//! (requestedExecutionLevel asInvoker + uiAccess="true"). Окна такого
//! процесса Windows размещает в защищённой z-полосе UIAccess — выше
//! CoreWindow Пуска/Поиска. Напоминание: exe с uiAccess=true обязан быть
//! подписан и лежать в защищённой папке — это делает make-uiaccess.ps1.
//!
//! Почему не всегда: uiAccess-манифест без подписи в защищённой папке даёт
//! exe, который вообще не запускается, поэтому обычная сборка должна
//! оставаться обычной.

use std::env;

fn main() {
    // Ранний выход: обычная сборка не должна зависеть от winresource/сборщиков.
    if env::var("KEYPRESS_UIACCESS").as_deref() != Ok("1") {
        return;
    }
    println!("cargo:rerun-if-env-changed=KEYPRESS_UIACCESS");
    println!("cargo:rerun-if-changed=assets/keypress-uiaccess.manifest");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        println!(
            "cargo:warning=KEYPRESS_UIACCESS=1 применяется только к Windows-таргету, манифест пропущен"
        );
        return;
    }

    // Манифест вшиваем из строки, вкомпилированной на этапе сборки: если файл
    // в репо отсутствует, упадёт сама компиляция build.rs (ранняя диагностика).
    const MANIFEST: &str = include_str!("assets/keypress-uiaccess.manifest");

    winresource::WindowsResource::new()
        .set_manifest(MANIFEST)
        .compile()
        .expect(
            "не удалось вшить манифест UIAccess. Для таргета x86_64-pc-windows-gnu нужен \
             x86_64-w64-mingw32-windres в PATH; для MSVC — rc.exe из Windows SDK \
             (сборка из x64 Native Tools Command Prompt).",
        );
}
