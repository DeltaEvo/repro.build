use std::ffi::OsStr;
use std::iter;
use std::process;
use std::rc::Rc;

use fuser::FileType;

use super::inode;

pub const ROOT_DIRECTORY: &'static str = concat!('.', env!("CARGO_BIN_NAME"));

pub fn is_inside(table: &inode::Table, inode: inode::Inode) -> bool {
    let name = table.name(inode);

    // If there is no last parent it means that the inode
    // is a top level file that has no parents (e.g. /top)
    table.parents(inode).last().unwrap_or(name).to_str() == Some(ROOT_DIRECTORY)
}

struct OsStrEq(&'static str);

impl PartialEq<Rc<OsStr>> for OsStrEq {
    fn eq(&self, other: &Rc<OsStr>) -> bool {
        *self.0 == **other
    }
}

struct Matcher<'a>(&'a inode::Table, inode::Inode);

impl Matcher<'_> {
    fn matches(&self, path: &[&'static str]) -> bool {
        let name = self.0.name(self.1);
        let parents = self.0.parents(self.1);

        let expected = iter::once(name).chain(parents);

        iter::once(ROOT_DIRECTORY)
            .chain(path.iter().copied())
            .rev()
            .map(OsStrEq)
            .eq(expected)
    }
}

pub fn readdir(table: &inode::Table, inode: inode::Inode) -> Option<&'static [&'static str]> {
    let matcher = Matcher(table, inode);

    if matcher.matches(&[]) {
        Some(&["pid", "licorne"])
    } else if matcher.matches(&["licorne"]) {
        Some(&["magique"])
    } else {
        None
    }
}

pub fn file_type(table: &inode::Table, inode: inode::Inode) -> Option<FileType> {
    let file_types: &[(&[&'static str], _)] = &[
        (&[], FileType::Directory),
        (&["pid"], FileType::RegularFile),
        (&["licorne"], FileType::Directory),
        (&["licorne", "magique"], FileType::RegularFile),
    ];
    let matcher = Matcher(table, inode);

    file_types
        .iter()
        .find(|(path, _)| matcher.matches(path))
        .map(|(_, file_type)| file_type)
        .copied()
}

pub fn read(table: &inode::Table, inode: inode::Inode) -> Option<Vec<u8>> {
    let matcher = Matcher(table, inode);

    if matcher.matches(&["pid"]) {
        Some(format!("{}", process::id()).into_bytes())
    } else {
        None
    }
}

pub fn unlink(table: &inode::Table, parent: inode::Inode, name: &OsStr, reply: fuser::ReplyEmpty) {
    let matcher = Matcher(table, parent);

    if matcher.matches(&[]) {
        if name == "pid" {
            reply.ok();
            process::exit(0);
        }
    }
}
