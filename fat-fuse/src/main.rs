mod block_file;
mod cli;

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::sync::Arc;

use block_dev::BlockDevice;
use clap::Parser;
use fat::{FatFileSystem, ROOT};
use typed_bytesize::ByteSizeIec;

pub use self::{block_file::BlockFile, cli::Cli};

fn main() -> io::Result<()> {
    env_logger::init();

    let cli = Cli::parse();
    println!("source={:?}\ntarget={:?}", cli.source, cli.target);

    let disk_size = ByteSizeIec::gib(4).0;
    let fd = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(cli.out_dir.join("fs.img"))?;
    fd.set_len(disk_size)?;

    let block_dev: Arc<dyn BlockDevice> = Arc::new(BlockFile::new(fd));
    let mut fs = FatFileSystem::foramt(disk_size as usize, &block_dev);

    let usr_bin = ROOT
        .mkdir("usr", &mut fs)
        .and_then(|usr| usr.mkdir("bin", &mut fs))
        .unwrap();

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
        log::info!("app={app:?}");
        let mut host_file = File::open(cli.target.join(&app))?;
        let mut elf_data: Vec<u8> = Vec::new();
        host_file.read_to_end(&mut elf_data)?;

        let mut inode = usr_bin.create_file(&app, &mut fs).unwrap();
        inode.write_at(0, &elf_data, &mut fs);
    }

    Ok(())
}
