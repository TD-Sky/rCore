#![feature(iterator_try_collect)]

use std::fs;
use std::fs::File;
use std::io;
use std::io::{BufWriter, Write};

static TARGET_PATH: &str = "../user/target/riscv64gc-unknown-none-elf/release/";

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=../user/src/");
    println!("cargo:rerun-if-changed={TARGET_PATH}");
    insert_app_data()
}

fn insert_app_data() -> io::Result<()> {
    let mut fd = File::create("src/link_app.S").map(BufWriter::new)?;
    let apps: io::Result<Option<Vec<String>>> = fs::read_dir("../user/src/bin")?
        .map(|app| {
            app.map(|app| {
                app.file_name()
                    .to_str()
                    .and_then(|s| s.split_once('.'))
                    .map(|s| s.0.to_owned())
            })
        })
        .try_collect();
    let mut apps = apps?.unwrap();
    apps.sort();
    let len = apps.len();

    writeln!(
        fd,
        r#"
    .align 3
    .section .data
    .global _num_app
_num_app:
    .quad {len}"#,
    )?;

    for i in 0..len {
        writeln!(fd, r#"    .quad app_{i}_start"#)?;
    }
    writeln!(fd, r#"    .quad app_{}_end"#, len - 1)?;

    for (i, app) in apps.iter().enumerate() {
        println!("app_{i}: {app}");
        writeln!(
            fd,
            r#"
    .section .data
    .global app_{i}_start
    .global app_{i}_end
app_{i}_start:
    .incbin "{TARGET_PATH}{app}.bin"
app_{i}_end:"#
        )?;
    }

    Ok(())
}
