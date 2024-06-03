#[derive(Debug)]
pub enum Error {
    AlreadyExists,
    NotFound,
    IsADirectory,
    NotADirectory,
    DirectoryNotEmpty,
}
