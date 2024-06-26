mod cli;

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Read;
use std::sync::Arc;
use std::sync::Mutex;

use clap::Parser;
use cli::Cli;
use easy_fs::EasyFileSystem;
use easy_fs_fuse::BlockFile;

fn main() -> io::Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    println!("source={:?}\ntarget={:?}", cli.source, cli.target);

    let block_file = Arc::new(BlockFile(Mutex::new({
        let fd = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(cli.out_dir.join("fs.img"))?;
        fd.set_len(16 * 2048 * 512).unwrap();

        fd
    })));

    let efs = EasyFileSystem::new(block_file, 16 * 2048, 1);
    let root_inode = Arc::new(EasyFileSystem::root_inode(&efs));

    let apps = fs::read_dir(&cli.source)?
        .map(|app| {
            app.map(|app| {
                app.file_name()
                    .to_str()
                    .and_then(|fname| fname.split_once('.'))
                    .expect("source file name doesn't match `*.rs`")
                    .0
                    .to_owned()
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    for app in apps {
        println!("program: {app:?}");
        let mut host_file = File::open(cli.target.join(&app))?;
        let mut elf_data: Vec<u8> = Vec::new();
        host_file.read_to_end(&mut elf_data)?;

        let inode = root_inode.create(&app).unwrap();
        inode.write_at(0, &elf_data);
    }

    Ok(())
}
