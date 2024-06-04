use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

pub trait Path: ToOwned {
    /// Returns the Path without its final component, if there is one.
    ///
    /// This means it returns [`Some("")`] for relative paths with one component.
    ///
    /// Returns None if the path terminates in a root or prefix, or if it’s the empty string.
    fn parent(&self) -> Option<&Self>;

    fn is_absolute(&self) -> bool;

    /// 返回不以`/`结束、不包含相对项的绝对路径。
    ///
    /// # 参数
    ///
    /// `cwd`: 来自于[`ProcessControlBlockInner`]，为绝对路径，
    ///        且非根时不以`/`结束。
    fn canonicalize(&self, cwd: &Self) -> Option<Self::Owned>;

    /// 返回根目录下的路径，若为根目录则返回`None`。
    fn root_relative(&self) -> Option<&Self>;

    /// Returns the final component of the Path, if there is one.
    ///
    /// If the path is a normal file, this is the file name. If it’s the path of a directory, this is the directory name.
    ///
    /// Returns [`None`] if the path terminates in ...
    fn file_name(&self) -> Option<&Self>;

    /// 返回路径的`(父目录, 文件名)`
    fn parent_file(&self) -> Option<(&Self, &Self)>;

    fn is_relative(&self) -> bool {
        !self.is_absolute()
    }
}

impl Path for str {
    fn is_absolute(&self) -> bool {
        self.starts_with('/')
    }

    fn parent(&self) -> Option<&Self> {
        let parent = self.rsplit_once('/')?.0;
        if parent.is_empty() && self.is_absolute() {
            return None;
        }
        Some(parent)
    }

    fn canonicalize(&self, cwd: &Self) -> Option<Self::Owned> {
        if self == "/" {
            return Some(String::from("/"));
        }

        let mut cmps = Vec::new();
        if self.is_relative() {
            // 防止第一个`/`带来的空字符串的影响，
            // 尤其是只有`cwd == /`时。
            cmps.extend(cwd.split('/').filter(|s| !s.is_empty()));
        }

        for cmp in self.trim_start_matches('/').split('/') {
            match cmp {
                ".." => {
                    cmps.pop()?;
                }
                "." => (),
                "" => return None,
                s => cmps.push(s),
            }
        }
        cmps.insert(0, ""); // 在接下来的拼接中代表根目录

        Some(cmps.join("/"))
    }

    fn root_relative(&self) -> Option<&Self> {
        debug_assert!(self.is_absolute());

        (self != "/").then_some(self.trim_start_matches('/'))
    }

    //WARN: 暂时先假设路径不包含`.`与`..`
    fn file_name(&self) -> Option<&Self> {
        let file_name = self.rsplit_once('/')?.1;
        if file_name.is_empty() && self.is_absolute() {
            return None;
        }
        Some(file_name)
    }

    //WARN: 暂时先假设路径不包含`.`与`..`
    fn parent_file(&self) -> Option<(&Self, &Self)> {
        if self == "/" {
            return None;
        }

        self.rsplit_once('/')
            .map(|(p, f)| if p.is_empty() { ("/", f) } else { (p, f) })
    }
}
